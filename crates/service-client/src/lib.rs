//! A private JSON-lines connection. Dropping it stops the service.
#[cfg(windows)]
mod job;
mod session;
use serde_json::Value;
pub use session::Session;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

pub struct Client {
    #[cfg(windows)]
    job: std::sync::Arc<job::Job>,
    child: Child,
    input: Option<mpsc::Sender<Vec<u8>>>,
    writer: Option<thread::JoinHandle<()>>,
    replies: mpsc::Receiver<io::Result<Value>>,
    reader: Option<thread::JoinHandle<()>>,
}

impl Client {
    pub fn start(executable: &Path) -> io::Result<Self> {
        Self::start_with_args(executable, &[])
    }

    pub fn start_with_args(executable: &Path, arguments: &[String]) -> io::Result<Self> {
        Self::start_with_environment(executable, arguments, &[])
    }

    pub fn start_with_environment(
        executable: &Path,
        arguments: &[String],
        environment: &[(String, String)],
    ) -> io::Result<Self> {
        #[cfg(windows)]
        let job = std::sync::Arc::new(job::Job::new()?);
        let mut command = Command::new(executable);
        command.args(arguments);
        command.envs(environment.iter().map(|(key, value)| (key, value)));
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        #[cfg(windows)]
        if let Err(error) = job.attach(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        let mut input = child.stdin.take().expect("piped input");
        let output = child.stdout.take().expect("piped output");
        let (sender, replies) = mpsc::sync_channel(1);
        let errors = sender.clone();
        let (input_sender, writes) = mpsc::channel::<Vec<u8>>();
        let writer = thread::spawn(move || {
            while let Ok(bytes) = writes.recv() {
                if let Err(error) = input.write_all(&bytes).and_then(|_| input.flush()) {
                    let _ = errors.send(Err(error));
                    return;
                }
            }
        });
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = (&mut output)
                    .take(4 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut line);
                let reply = match result {
                    Ok(0) => Err(io::Error::other("service disconnected")),
                    Ok(_) if line.len() > 4 * 1024 * 1024 => {
                        Err(io::Error::other("service reply too large"))
                    }
                    Ok(_) => serde_json::from_slice(&line)
                        .map_err(|_| io::Error::other("invalid service JSON")),
                    Err(error) => Err(error),
                };
                let failed = reply.is_err();
                if sender.send(reply).is_err() || failed {
                    return;
                }
            }
        });
        Ok(Self {
            #[cfg(windows)]
            job,
            child,
            input: Some(input_sender),
            writer: Some(writer),
            replies,
            reader: Some(reader),
        })
    }

    pub fn request(&mut self, request: &Value) -> io::Result<Value> {
        self.request_cancellable(request, &AtomicBool::new(false))
    }

    pub fn request_cancellable(
        &mut self,
        request: &Value,
        cancelled: &AtomicBool,
    ) -> io::Result<Value> {
        if cancelled.load(Ordering::Acquire) {
            return Err(io::Error::other("request cancelled"));
        }
        let mut bytes = serde_json::to_vec(request)?;
        if bytes.len() > 65536 || !request["id"].is_string() {
            return Err(io::Error::other("invalid client request"));
        }
        let timeout = if request["method"] == "lifecycle.suspend" {
            Duration::from_secs(8)
        } else {
            Duration::from_secs(60)
        };
        let deadline = std::time::Instant::now() + timeout;
        bytes.push(b'\n');
        self.input
            .as_ref()
            .ok_or_else(|| io::Error::other("service stopped"))?
            .send(bytes)
            .map_err(|_| io::Error::other("service disconnected"))?;
        let reply = loop {
            if cancelled.load(Ordering::Acquire) || std::time::Instant::now() >= deadline {
                #[cfg(windows)]
                self.job.stop();
                let _ = self.child.kill();
                return Err(io::Error::other("service request cancelled or timed out"));
            }
            match self.replies.recv_timeout(Duration::from_millis(25)) {
                Ok(reply) => break reply?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(io::Error::other("service disconnected")),
            }
        };
        if reply["version"] != 1 || reply["id"] != request["id"] || !reply["ok"].is_boolean() {
            return Err(io::Error::other("service protocol mismatch"));
        }
        Ok(reply)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        #[cfg(windows)]
        self.job.stop();
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Closing the receiver releases a writer waiting on an unsolicited reply.
        let (_, empty) = mpsc::channel();
        self.replies = empty;
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

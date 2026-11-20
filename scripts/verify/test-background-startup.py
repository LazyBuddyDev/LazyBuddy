"""A GUI host survives closing its owned hidden launcher console; no startup writes."""
import argparse, ctypes, json, os, shutil, struct, subprocess, sys, tempfile, time, uuid, winreg
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

if len(sys.argv) > 1 and sys.argv[1] == '--parent':
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.GetConsoleWindow.restype = ctypes.c_void_p
    child = subprocess.Popen([sys.argv[3], '--background'], cwd=os.environ['SystemRoot'] + '/System32')
    processes = (ctypes.c_ulong * 32)()
    count = kernel.GetConsoleProcessList(processes, 32)
    Path(sys.argv[2]).write_text(json.dumps(dict(pid=child.pid, console=kernel.GetConsoleWindow(), attached=list(processes)[:count])))
    child.wait()
    sys.exit(0)

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host-dir', type=Path, required=True)
args = parser.parse_args()
binaries = args.host_dir.resolve()
host, cli = binaries / 'lazybuddy.exe', binaries / 'lazybuddy-cli.exe'
with host.open('rb') as binary:
    assert binary.read(2) == b'MZ'
    binary.seek(0x3c)
    offset = struct.unpack('<I', binary.read(4))[0]
    binary.seek(offset)
    assert binary.read(4) == b'PE\0\0'
    binary.seek(offset + 24 + 68)
    subsystem = struct.unpack('<H', binary.read(2))[0]
assert subsystem == 2, f'Desktop host must use GUI subsystem in every build, got {subsystem}'

def registered_command():
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r'Software\Microsoft\Windows\CurrentVersion\Run') as key:
            return winreg.QueryValueEx(key, 'LazyBuddy')
    except FileNotFoundError:
        return None

registered_before = registered_command()
profiles = ROOT / 'tmp/profiles'
profiles.mkdir(parents=True, exist_ok=True)
profile = Path(tempfile.mkdtemp(prefix='startup-', dir=profiles))
data = profile / 'data'
data.mkdir()
(data / 'web-preferences.json').write_text(json.dumps(dict(auto_update=False)), encoding='utf-8')
env = dict(os.environ, LAZYBUDDY_INSTANCE='qa-startup-' + uuid.uuid4().hex[:12],
           LAZYBUDDY_DATA_DIR=str(data), WEBVIEW2_USER_DATA_FOLDER=str(profile / 'webview'),
           LAZYBUDDY_PROTOTYPE_PROBE='1')

def rpc(command):
    result = subprocess.run([str(cli), 'host', json.dumps(dict(version=1, command=command, args={}))],
                            env=env, capture_output=True, text=True, encoding='utf-8', timeout=10,
                            creationflags=subprocess.CREATE_NO_WINDOW)
    return json.loads(result.stdout) if result.returncode == 0 else None

startup = subprocess.STARTUPINFO()
startup.dwFlags = subprocess.STARTF_USESHOWWINDOW
startup.wShowWindow = 0
parent = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), '--parent', str(profile / 'parent.json'), str(host)],
                          env=env, startupinfo=startup, creationflags=subprocess.CREATE_NEW_CONSOLE)
try:
    deadline = time.monotonic() + 35
    while not (status := rpc('status')):
        assert parent.poll() is None and time.monotonic() < deadline, 'owned background host did not become ready'
        time.sleep(.1)
    state = status['data']
    assert status['ok'] and state['hidden'] is True
    launcher = json.loads((profile / 'parent.json').read_text())
    assert launcher['pid'] == state['pid'] and launcher['console']
    assert state['pid'] not in launcher['attached'], 'desktop host inherited launcher console'
    user = ctypes.WinDLL('user32', use_last_error=True)
    user.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
    assert user.PostMessageW(launcher['console'], 0x0010, 0, 0), 'owned hidden console close failed'
    parent.wait(timeout=15)
    time.sleep(2)
    current = rpc('status')
    assert current and current['ok'] and current['data']['pid'] == state['pid'] and current['data']['hidden']
    assert registered_command() == registered_before, 'isolated check changed production startup'
    report = dict(passed=True, gui_subsystem=subsystem, background_ready=True,
                  survived_launcher_console_close=True, stayed_hidden=True, startup_registry_changed=False,
                  real_login_cycle_tested=False)
finally:
    if rpc('status'):
        assert rpc('app.quit')['ok']
    deadline = time.monotonic() + 30
    while rpc('status'):
        assert time.monotonic() < deadline, 'owned host did not quit normally'
        time.sleep(.1)
    parent.wait(timeout=15)
    assert profile.resolve().is_relative_to(profiles.resolve())
    deadline = time.monotonic() + 15
    while profile.exists():
        try:
            shutil.rmtree(profile)
        except OSError as error:
            if error.winerror not in (2, 3, 5, 32, 145) or time.monotonic() >= deadline:
                raise
            time.sleep(.2)
output = ROOT / 'tmp/verification-startup'
output.mkdir(exist_ok=True)
(output / 'gui-background.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report))

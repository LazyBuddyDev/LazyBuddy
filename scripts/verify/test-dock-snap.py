"""Check capsule window styles and restoration without moving the physical mouse."""
import argparse,json,os,shutil,subprocess,sys,tempfile,time,uuid
from pathlib import Path

ROOT=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host-dir',type=Path,required=True)
binaries=parser.parse_args().host_dir.resolve()
with tempfile.TemporaryDirectory(prefix='dock-snap-',dir=ROOT/'tmp/profiles',ignore_cleanup_errors=True) as temporary:
    root=Path(temporary);data=root/'data';data.mkdir()
    (data/'web-preferences.json').write_text(json.dumps(dict(auto_update=False)),encoding='utf-8')
    env=dict(os.environ,LAZYBUDDY_INSTANCE='qa-dock-snap-'+uuid.uuid4().hex,
             LAZYBUDDY_DATA_DIR=str(data),LAZYBUDDY_PROTOTYPE_PROBE='1',WEBVIEW2_USER_DATA_FOLDER=str(root/'webview'))
    child=subprocess.Popen([str(binaries/'lazybuddy.exe'),'--background'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
    def rpc(command):
        result=subprocess.run([str(binaries/'lazybuddy-cli.exe'),'host',json.dumps(dict(version=1,command=command,args={}))],env=env,capture_output=True,encoding='utf-8',timeout=10,creationflags=subprocess.CREATE_NO_WINDOW)
        return json.loads(result.stdout) if result.returncode==0 else None
    def native(action='snapshot'):
        result=subprocess.check_output([sys.executable,'-B',str(ROOT/'apps/desktop/tests/native-dock.py'),'--pid',str(child.pid),'--action',action],encoding='utf-8',timeout=15,creationflags=subprocess.CREATE_NO_WINDOW)
        return json.loads(result)
    try:
        deadline=time.monotonic()+20
        while not rpc('status'):
            assert child.poll() is None and time.monotonic()<deadline
            time.sleep(.1)
        assert rpc('window.show')['ok']
        original=native();assert original['visible'];cycles=[]
        for _ in range(3):
            left=native('drag-left');right=native('drag-right');left_again=native('drag-left')
            for capsule in (left,right,left_again):
                assert not capsule['resizable'] and not capsule['maximizable'],capsule
                assert capsule['visible'], 'capsule must remain visible after native style changes'
                assert capsule['after'][2]==capsule['after'][3]==round(40*capsule['dpi']/96)
                assert capsule['region']['center'] and not capsule['region']['corner']
            assert rpc('window.show')['ok']
            restored=native()
            assert restored['visible']
            assert restored['resizable']==original['resizable'] and restored['maximizable']==original['maximizable']
            assert restored['after'][2:]==original['after'][2:]
            assert restored['topmost']==original['topmost']
            cycles.append(dict(left=left,right=right,left_again=left_again,restored=restored))
        report=dict(passed=True,original=original,cycles=cycles,physical_mouse_used=False)
    finally:
        if child.poll() is None:
            assert rpc('app.quit')['ok'];child.wait(timeout=30)
    output=ROOT/'tmp/verification-dock-snap';output.mkdir(exist_ok=True)
    (output/'native.json').write_text(json.dumps(report,indent=2),encoding='utf-8')

# WebView2 may retain its profile briefly after the owned host exits.
deadline=time.monotonic()+20
while root.exists():
    try:shutil.rmtree(root)
    except PermissionError:
        if time.monotonic()>=deadline:raise
        time.sleep(.2)
print('PASS: 3 left/right/left capsule cycles disable resizing/maximizing and restore original capabilities, dimensions and topmost state.')

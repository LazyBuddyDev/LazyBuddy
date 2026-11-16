"""Reject signed application updates in an owned Debug host; no real account or installation."""
import argparse,json,os,shutil,subprocess,tempfile,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host-dir',type=Path,required=True)
args=parser.parse_args();binaries=args.host_dir.resolve()
with tempfile.TemporaryDirectory(prefix='qa-development-updates-',dir=ROOT/'tmp',ignore_cleanup_errors=True) as temporary:
    root=Path(temporary);data=root/'data';data.mkdir();preferences=data/'web-preferences.json'
    preferences.write_text(json.dumps(dict(auto_update=True,dark=True)),encoding='utf-8')
    env=dict(os.environ,LAZYBUDDY_INSTANCE='qa-dev-updates-'+uuid.uuid4().hex,LAZYBUDDY_DATA_DIR=str(data),
             LAZYBUDDY_PROTOTYPE_PROBE='1',WEBVIEW2_USER_DATA_FOLDER=str(root/'webview'))
    child=subprocess.Popen([str(binaries/'lazybuddy.exe'),'--background'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
    def rpc(command):
        result=subprocess.run([str(binaries/'lazybuddy-cli.exe'),'host',json.dumps(dict(version=1,command=command,args={}))],env=env,capture_output=True,encoding='utf-8',timeout=10,creationflags=subprocess.CREATE_NO_WINDOW)
        return json.loads(result.stdout) if result.returncode==0 else None
    try:
        deadline=time.monotonic()+20
        while not (status:=rpc('status')):
            assert child.poll() is None and time.monotonic()<deadline
            time.sleep(.1)
        assert status['data']['app_updates'] is False, 'requires a Debug host'
        for command in ['updates.check','updates.apply']:
            result=rpc(command);assert result['ok'] is False
            assert 'build_and_run.bat' in json.dumps(result,ensure_ascii=False)
        status=rpc('status')['data'];assert not status['update_checking'] and not status['update_ready']
        assert not (data/'updates').exists(), 'no downloaded or prepared update cache'
        assert json.loads(preferences.read_text())['auto_update'] is True
        report=dict(passed=True,app_updates_disabled=True,check_and_apply_rejected=True,update_cache_absent=True,auto_update_preference_preserved=True,real_accounts_accessed=False)
    finally:
        if child.poll() is None:
            assert rpc('app.quit')['ok'];child.wait(timeout=30)
    output=ROOT/'tmp/verification-development-updates';output.mkdir(exist_ok=True)
    (output/'native.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))

# WebView2 may retain its profile briefly after the owned host exits.
deadline=time.monotonic()+20
while root.exists():
    try:shutil.rmtree(root)
    except PermissionError:
        if time.monotonic()>=deadline:raise
        time.sleep(.2)

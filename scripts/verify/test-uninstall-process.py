"""Uninstall only owned temporary installations; never access real accounts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--host-dir', type=Path, required=True)
args = parser.parse_args()
source = args.host_dir.resolve()
reports = []

for launch, purge in ((False, False), (True, False), (True, True)):
    with tempfile.TemporaryDirectory(prefix='qa-uninstall-', dir=ROOT/'tmp') as temporary:
        root = Path(temporary)
        install = root/'install'
        install.mkdir()
        data = root/'data'
        data.mkdir()
        scratch = root/'temp'
        scratch.mkdir()
        if (source/'build-info.json').is_file():
            # Model the ZIP/update installation, not empty build-only directories.
            info = json.loads((source/'build-info.json').read_text(encoding='utf-8'))
            for name in [*info['files'], 'build-info.json']:
                destination = install/name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(source/name, destination)
        else:
            for name in ('lazybuddy.exe', 'lazybuddy-cli.exe', 'lazybuddy-updater.exe', 'uninstall.exe', 'module-catalog.json', 'app-update-source.json'):
                shutil.copyfile(source/name, install/name)
            files = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in install.iterdir()}
            build = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
            (install/'build-info.json').write_text(json.dumps(dict(schema=1, version='2.0.1', build=build, files=files)), encoding='utf-8')
        assert (install/'uninstall.exe').is_file(), 'uninstall entry must exist before first launch'
        (install/'keep.txt').write_text('user document', encoding='utf-8')
        (data/'web-preferences.json').write_text(json.dumps(dict(auto_update=False, dark=True)), encoding='utf-8')
        (data/'account-sentinel').write_text('fixture only; no credential access', encoding='utf-8')
        env = dict(os.environ, LAZYBUDDY_INSTANCE='qa-uninstall-'+uuid.uuid4().hex,
                   LAZYBUDDY_DATA_DIR=str(data), TEMP=str(scratch), TMP=str(scratch),
                   WEBVIEW2_USER_DATA_FOLDER=str(root/'webview'))
        cli = source/'lazybuddy-cli.exe'

        def call(*arguments, success=True):
            result = subprocess.run([str(cli), *arguments], env=env, capture_output=True,
                                    text=True, encoding='utf-8', timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
            assert (result.returncode == 0) == success, result.stderr
            return json.loads(result.stdout) if success else None

        # Local inert package: installed but never launched or connected to upstream.
        feature = root/'feature'
        (feature/'bin').mkdir(parents=True)
        shutil.copyfile(ROOT/'contracts/fixtures/feature.json', feature/'module.json')
        (feature/'bin/service.exe').write_bytes(b'not executed')
        runtime = root/'runtime'
        (runtime/'bin').mkdir(parents=True)
        (runtime/'bin/engine.exe').write_bytes(b'not executed')
        (runtime/'module.json').write_text(json.dumps(dict(schema=1,service_api=1,id='test-runtime',name='test runtime',version='0.1.0',target='windows-x86_64',kind='runtime',page=None,entrypoint='bin/engine.exe',dependencies=[])),encoding='utf-8')
        call('modules','install-local',str(runtime))
        manifest=json.loads((feature/'module.json').read_text(encoding='utf-8'))
        manifest['dependencies']=[dict(id='test-runtime',version='0.1.0')]
        (feature/'module.json').write_text(json.dumps(manifest),encoding='utf-8')
        call('modules', 'install-local', str(feature))
        host = subprocess.Popen([str(install/'lazybuddy.exe'), '--background'], env=env,
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                creationflags=subprocess.CREATE_NO_WINDOW) if launch else None
        try:
            deadline = time.monotonic()+25
            while host:
                assert host.poll() is None, 'owned host exited'
                status = subprocess.run([str(cli), 'host', json.dumps(dict(version=1, command='status', args={}))], env=env,
                                        capture_output=True, text=True, encoding='utf-8', timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
                if status.returncode == 0 and json.loads(status.stdout).get('data', {}).get('pid') == host.pid:
                    break
                assert time.monotonic() < deadline, 'owned host not ready'
                time.sleep(.1)
            assert hashlib.sha256((install/'uninstall.exe').read_bytes()).hexdigest() == json.loads((install/'build-info.json').read_text(encoding='utf-8'))['files']['uninstall.exe']
            command = [str(install/'uninstall.exe'), '--yes']
            if purge:
                command.append('--remove-modules')
            subprocess.run(command, env=env, check=True, timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
            if host:
                host.wait(timeout=35)
            deadline = time.monotonic()+30
            while (install/'lazybuddy.exe').exists() or list(scratch.iterdir()):
                errors = list(scratch.glob('*/error.txt'))
                assert not errors, [p.read_text(encoding='utf-8') for p in errors]
                assert time.monotonic() < deadline, 'uninstall or self-cleanup timed out'
                time.sleep(.1)
            # The cleanup shell can still hold TEMP as its cwd just after removing its files.
            while scratch.exists():
                try:
                    scratch.rmdir()
                except OSError:
                    assert time.monotonic() < deadline, 'cleanup shell did not release TEMP'
                    time.sleep(.1)
            remaining = sorted(p.name for p in install.iterdir())
            assert remaining == ['keep.txt'], remaining
            assert (install/'keep.txt').read_text(encoding='utf-8') == 'user document'
            assert (data/'account-sentinel').exists()
            assert json.loads((data/'web-preferences.json').read_text(encoding='utf-8'))['dark'] is True
            packages = call('modules', 'list')
            assert sorted(row['id'] for row in packages) == ([] if purge else ['sample','test-runtime'])
            reports.append(dict(host_started=launch,remove_modules=purge, normal_exit=True, owned_files_removed=True,
                                user_files_preserved=True, settings_preserved=True,
                                account_data_preserved=True, helper_cleaned=True))
        finally:
            if host and host.poll() is None:
                call('host', json.dumps(dict(version=1, command='app.quit', args={})))
                host.wait(timeout=20)

output = ROOT/'tmp/verification-uninstall'
output.mkdir(exist_ok=True)
(output/'uninstall.json').write_text(json.dumps(reports, indent=2), encoding='utf-8')
print(json.dumps(dict(passed=True, cases=reports)))

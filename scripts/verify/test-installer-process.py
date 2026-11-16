"""Exercise the real installer with a private registry key and temporary install/data."""
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
import winreg
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from package_installer import payload_files
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--bundle', type=Path, required=True)
parser.add_argument('--compiler', type=Path, required=True)
args = parser.parse_args()
payload = args.bundle.resolve() / 'LazyBuddy'
version = json.loads((payload / 'build-info.json').read_text(encoding='utf-8'))['version']
root = Path(tempfile.mkdtemp(prefix='qa-installer-process-', dir=ROOT / 'tmp')).resolve()
assert root.is_relative_to((ROOT / 'tmp').resolve())
instance = 'qa-installer-' + uuid.uuid4().hex
key = 'Software\\LazyBuddy\\Verification\\' + instance
installer = root / 'LazyBuddy-QA.exe'
install = root / 'installed'
data = root / 'data'
scratch = root / 'temp'
data.mkdir(); scratch.mkdir()
(data / 'settings-sentinel').write_text('keep settings')
(data / 'account-sentinel').write_text('fixture, no credential access')
env = dict(os.environ, LAZYBUDDY_INSTANCE=instance, LAZYBUDDY_DATA_DIR=str(data),
           TEMP=str(scratch), TMP=str(scratch))
subprocess.run([str(args.compiler.resolve()), '/V2', f'/DPAYLOAD={payload}',
                f'/DPAYLOAD_FILES={payload_files(payload, root / "files.nsh")}',
                f'/DOUTPUT={installer}', f'/DVERSION={version}', f'/DREGKEY={key}',
                str(ROOT / 'scripts/installer.nsi')], check=True)
def run(*command):
    return subprocess.run(command, env=env, timeout=45, creationflags=subprocess.CREATE_NO_WINDOW)

assert run(str(installer), '/S', '/D=' + str(install)).returncode == 0
info = json.loads((install / 'build-info.json').read_text(encoding='utf-8'))
assert info['version'] == version
for name, expected in info['files'].items():
    assert hashlib.sha256((install / name).read_bytes()).hexdigest() == expected, name
with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key, 0, winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as registration:
    assert winreg.QueryValueEx(registration, 'InstallLocation')[0] == str(install)
    assert winreg.QueryValueEx(registration, 'DisplayVersion')[0] == version
    assert winreg.QueryValueEx(registration, 'UninstallString')[0] == f'"{install}\\uninstall.exe"'
(install / 'keep.txt').write_text('user file')
assert run(str(installer), '/S', '/D=' + str(install)).returncode != 0
assert (install / 'keep.txt').read_text() == 'user file'
other = root / 'other'
assert run(str(installer), '/S', '/D=' + str(other)).returncode != 0
assert not other.exists(), 'existing registry entry must not be overwritten'
assert run(str(install / 'uninstall.exe'), '--yes').returncode == 0
deadline = time.monotonic() + 35
while (install / 'lazybuddy.exe').exists() or list(scratch.iterdir()):
    errors = list(scratch.glob('*/error.txt'))
    assert not errors, [p.read_text() for p in errors]
    assert time.monotonic() < deadline, 'uninstall cleanup timed out'
    time.sleep(.1)
assert sorted(p.name for p in install.iterdir()) == ['keep.txt']
assert (data / 'account-sentinel').exists() and (data / 'settings-sentinel').exists()
try:
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
        raise AssertionError('owned uninstall registration remains')
except FileNotFoundError:
    pass
report = dict(passed=True, version=info['version'], all_payload_hashes_match=True,
              owned_registration_removed=True, existing_directory_rejected=True,
              second_install_rejected=True, user_files_and_data_preserved=True,
              real_user_installation_accessed=False)
output = ROOT / 'tmp/verification-installer'
output.mkdir(exist_ok=True)
(output / 'installer.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
# Only the verified temporary directory created by this invocation is removed.
assert root.resolve().is_relative_to((ROOT / 'tmp').resolve()) and root.name.startswith('qa-installer-process-')
for attempt in range(30):
    try:
        shutil.rmtree(root)
        break
    except PermissionError:
        if attempt == 29:
            raise
        time.sleep(.1)
print(json.dumps(report))

"""Wrap a verified base bundle in a Windows installation wizard (NSIS 3.11)."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

from package_base import ROOT, application_manifest


def payload_files(payload: Path, output: Path):
    lines = []
    for path in sorted(payload.rglob('*')):
        if not path.is_file():
            continue
        name = str(path.relative_to(payload))
        if any(char in name for char in '$"\r\n'):
            raise ValueError('unsupported NSIS payload filename')
        parent = str(path.relative_to(payload).parent)
        lines.extend([f'SetOutPath "$INSTDIR\\{parent}"', f'File "${{PAYLOAD}}\\{name}"'])
    output.write_text('\n'.join(lines), encoding='utf-8-sig')
    return output


def build_installer(bundle: Path, compiler: Path):
    archive = bundle / 'LazyBuddy-windows-x86_64.zip'
    manifest = application_manifest(archive, 'https://example.invalid/app.zip')
    payload = bundle / 'LazyBuddy'
    # Compile only the exact staged files already checked in the update archive.
    with zipfile.ZipFile(archive) as package:
        expected = {name.removeprefix('LazyBuddy/'): package.read(name) for name in package.namelist()}
    actual = {p.relative_to(payload).as_posix(): p.read_bytes() for p in payload.rglob('*') if p.is_file()}
    if actual != expected or any(p.is_symlink() for p in payload.rglob('*')):
        raise ValueError('installer payload differs from verified base bundle')
    output = bundle / 'LazyBuddy-Setup-windows-x86_64.exe'
    listing = payload_files(payload, bundle / 'installer-files.nsh')
    subprocess.run([str(compiler), '/V2', f'/DPAYLOAD={payload}', f'/DOUTPUT={output}',
                    f'/DPAYLOAD_FILES={listing}',
                    f'/DVERSION={manifest["version"]}', str(ROOT / 'scripts/installer.nsi')], check=True)
    result = dict(installer=output.name, version=manifest['version'], build=manifest['build'],
                  bytes=output.stat().st_size, sha256=hashlib.sha256(output.read_bytes()).hexdigest())
    (bundle / 'installer.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--compiler', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build_installer(args.bundle.resolve(), args.compiler.resolve())))

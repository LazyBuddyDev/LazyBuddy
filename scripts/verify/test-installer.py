"""Installer payload must match the verified update bundle byte for byte."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from package_base import write_archive
from package_installer import build_installer


class Installer(unittest.TestCase):
    def test_installer_uses_exact_verified_payload_and_version(self):
        with tempfile.TemporaryDirectory(prefix='qa-installer-', dir=ROOT / 'tmp') as temporary:
            output = Path(temporary)
            payload = output / 'LazyBuddy'
            payload.mkdir()
            for name in ['lazybuddy.exe', 'lazybuddy-cli.exe', 'lazybuddy-updater.exe', 'uninstall.exe',
                         'app-update-source.json', 'module-catalog.json']:
                (payload / name).write_bytes(name.encode())
            files = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in payload.iterdir()}
            build = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
            (payload / 'build-info.json').write_text(json.dumps(dict(schema=1, version='1.0.0', build=build, files=files)))
            write_archive(payload, output)
            def compile(args, **kwargs):
                self.assertIn('/DVERSION=1.0.0', args)
                self.assertTrue(kwargs['check'])
                (output / 'LazyBuddy-Setup-windows-x86_64.exe').write_bytes(b'fixture installer')
            with patch('package_installer.subprocess.run', side_effect=compile) as run:
                result = build_installer(output, Path('makensis.exe'))
                self.assertEqual(result['version'], '1.0.0')
                self.assertEqual(result['build'], build)
                self.assertEqual(run.call_count, 1)
                (payload / 'unexpected.txt').write_text('must not enter installer')
                with self.assertRaises(ValueError):
                    build_installer(output, Path('makensis.exe'))
                self.assertEqual(run.call_count, 1)
                (payload / 'unexpected.txt').unlink()
                (payload / 'lazybuddy.exe').write_bytes(b'tampered')
                with self.assertRaises(ValueError):
                    build_installer(output, Path('makensis.exe'))


if __name__ == '__main__':
    unittest.main()

"""DPAPI identity reuse, overwrite protection and tamper rejection; no release key access."""
from pathlib import Path
import importlib.util
import shutil
import sys
import tempfile
import unittest
import json
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from windows_signing_key import load_key, protect
ROOT = Path(__file__).resolve().parents[2]
TMP = ROOT / 'tmp'
TMP.mkdir(exist_ok=True)


class SigningKey(unittest.TestCase):
    def test_shipped_catalog_matches_trusted_public_key(self):
        from urllib.parse import urlsplit
        source = json.loads((ROOT / 'module-catalog.json').read_text(encoding='utf-8'))
        path=ROOT/'docs/modules'/Path(urlsplit(source['url']).path).name
        self.assertTrue(path.is_file(), 'active catalog is missing')
        key=Ed25519PublicKey.from_public_bytes(bytes(source['public_key']))
        key.verify(path.with_name(path.name+'.sig').read_bytes(),path.read_bytes())

    def test_reuse_and_reject_overwrite_or_corruption(self):
        with tempfile.TemporaryDirectory(prefix='lazybuddy-signing-test-',dir=TMP) as directory:
            path = Path(directory) / 'test.dpapi'
            checkout = Path(directory) / 'checkout'
            script = checkout / 'scripts/windows_signing_key.py'
            script.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / 'scripts/windows_signing_key.py', script)
            spec = importlib.util.spec_from_file_location('fixture_signing_key', script)
            fixture = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(fixture)
            first = fixture.load_key(path, initialize=True)
            second = fixture.load_key(path)
            second.public_key().verify(first.sign(b'catalog'), b'catalog')
            with self.assertRaises(FileExistsError):
                fixture.load_key(path, initialize=True)
            with self.assertRaises(ValueError):
                fixture.load_key(checkout / 'forbidden.dpapi', initialize=True)
            corrupted = bytearray(path.read_bytes())
            corrupted[-1] ^= 1
            with self.assertRaises(OSError):
                protect(bytes(corrupted), decrypt=True)

    def test_reject_checkout_private_key(self):
        with self.assertRaises(ValueError):
            load_key(Path(__file__).resolve().parents[2] / 'target' / 'forbidden.dpapi')


if __name__ == '__main__':
    unittest.main()

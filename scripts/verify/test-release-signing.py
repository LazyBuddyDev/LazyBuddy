"""Central signing boundary with ephemeral test keys; never reads a release key."""
import base64,importlib.util,json,sys,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('release_signing',ROOT/'scripts/release-signing.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
class Signing(unittest.TestCase):
 def fixture(self,root,key):
  for name in ('module-catalog.json','app-update-source.json'):(root/name).write_text(json.dumps({'public_key':list(key.public_key().public_bytes_raw())}))
  (root/'apps/desktop/src-tauri').mkdir(parents=True);(root/'apps/desktop/src-tauri/Cargo.toml').write_text('[package]\nversion="1.0.8"')
  v=dict(schema=1,target='windows-x86_64',version='1.0.8',build='a'*64,url='https://github.com/LazyBuddyDev/LazyBuddy/releases/download/v1.0.8/LazyBuddy-windows-x86_64.zip',bytes=10,sha256='b'*64,host_sha256='c'*64)
  path=root/'update.json';path.write_text(json.dumps(v));return path,v
 def test_exact_payload_and_only_public_outputs(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'tmp') as d:
   root=Path(d);key=Ed25519PrivateKey.generate();path,_=self.fixture(root,key);output=root/'signed'
   m.sign(key,None,path,output,root);self.assertEqual((output/'app-update.json').read_bytes(),path.read_bytes())
   key.public_key().verify((output/'app-update.json.sig').read_bytes(),path.read_bytes());self.assertEqual(sorted(p.name for p in output.iterdir()),['app-update.json','app-update.json.sig'])
 def test_wrong_identity_never_uploads_or_writes(self):
  key=Ed25519PrivateKey.generate();other=Ed25519PrivateKey.generate()
  with patch.object(m.subprocess,'run') as run:
   with self.assertRaises(ValueError):m.register(key,other.public_key().public_bytes_raw(),'gh')
   run.assert_not_called()
 def test_registration_sends_key_only_through_stdin(self):
  key=Ed25519PrivateKey.generate()
  with patch.object(m.subprocess,'run') as run:
   run.return_value.returncode=0;m.register(key,key.public_key().public_bytes_raw(),'gh')
   args=run.call_args;self.assertNotIn(args.kwargs['input'].decode(),str(args.args[0]));self.assertEqual(args.args[0][-2:],['--env','release-signing']);self.assertTrue(args.kwargs['capture_output'])
 def test_registration_checks_both_remote_trust_roots(self):
  public=Ed25519PrivateKey.generate().public_key().public_bytes_raw()
  def response(value):
   from types import SimpleNamespace
   return SimpleNamespace(returncode=0,stdout=base64.b64encode(json.dumps({'public_key':list(value)}).encode()))
  with patch.object(m.subprocess,'run',side_effect=[response(public),response(public)]):self.assertEqual(m.remote_public('gh'),public)
  other=Ed25519PrivateKey.generate().public_key().public_bytes_raw()
  with patch.object(m.subprocess,'run',side_effect=[response(public),response(other)]):
   with self.assertRaises(ValueError):m.remote_public('gh')
 def test_bad_update_url_version_and_duplicate_fields_are_rejected(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'tmp') as d:
   root=Path(d);_,v=self.fixture(root,Ed25519PrivateKey.generate())
   for key,value in [('url','https://example.com/payload.zip'),('version','9.0.0'),('bytes',True),('host_sha256','bad')]:
    bad=dict(v);bad[key]=value
    with self.assertRaises(ValueError):m.validate(json.dumps(bad).encode(),'update','1.0.8')
   with self.assertRaises(ValueError):m.validate(b'{"schema":1,"schema":1}','update','1.0.8')
 def test_update_sequence_has_a_bounded_integer_value(self):
  with tempfile.TemporaryDirectory(dir=ROOT/'tmp') as d:
   _,update=self.fixture(Path(d),Ed25519PrivateKey.generate())
   m.validate(json.dumps(dict(update,sequence=42)).encode(),'update','1.0.8')
   for sequence in (0,True,-1,2**64,None):
    with self.assertRaises(ValueError):
     m.validate(json.dumps(dict(update,sequence=sequence)).encode(),'update','1.0.8')
 def test_catalog_dependencies_and_hosts_are_checked(self):
  v=json.loads((ROOT/'docs/modules/stable-v6-catalog.json').read_bytes());m.validate(json.dumps(v).encode(),'catalog','1.0.8')
  v['packages'][0]['url']='https://example.com/untrusted.zip'
  with self.assertRaises(ValueError):m.validate(json.dumps(v).encode(),'catalog','1.0.8')
  v=json.loads((ROOT/'docs/modules/stable-v6-catalog.json').read_bytes());v['packages']=[p for p in v['packages'] if p['id']!='python-runtime']
  with self.assertRaises(ValueError):m.validate(json.dumps(v).encode(),'catalog','1.0.8')
if __name__=='__main__':unittest.main()

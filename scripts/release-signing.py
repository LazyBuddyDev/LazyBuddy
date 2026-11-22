"""One release identity for all PCs; only public payloads leave the signing runner."""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib
from urllib.parse import urlsplit
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'LazyBuddyDev/LazyBuddy'
ENVIRONMENT = 'release-signing'
KEY_ENV = 'LAZYBUDDY_RELEASE_SIGNING_KEY'


def trusted_public(root=ROOT):
    module = json.loads((root/'module-catalog.json').read_text(encoding='utf-8'))['public_key']
    app = json.loads((root/'app-update-source.json').read_text(encoding='utf-8'))['public_key']
    if module != app or len(module) != 32 or any(type(n) is not int or not 0 <= n <= 255 for n in module):
        raise ValueError('Shipped trust roots differ or are invalid')
    return bytes(module)


def verify_identity(key, public):
    if key.public_key().public_bytes_raw() != public:
        raise ValueError('This key is not the current release identity; use the PC holding the matching key')


def github_asset(url):
    u = urlsplit(url)
    return (u.scheme == 'https' and u.netloc == 'github.com' and not u.query and not u.fragment
        and re.fullmatch(r'/LazyBuddyDev/(LazyBuddy|news-service|hongguo-service|reader-service|bilibili-service)/releases/download/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+\.zip', u.path) is not None)


def digest(value):
    return isinstance(value, str) and re.fullmatch('[a-f0-9]{64}', value) is not None


def package_version(value):
    if not isinstance(value,str):
        return False
    parts = value.split('.')
    return len(parts) == 3 and all(
        re.fullmatch(r'0|[1-9][0-9]{0,9}', part) and int(part) <= 2**32-1
        for part in parts
    )


def validate(payload, kind, version):
    if len(payload) > 1024*1024:
        raise ValueError('Signing payload exceeds limit')
    def pairs(rows):
        out = {}
        for k,v in rows:
            if k in out:raise ValueError('Duplicate JSON field')
            out[k] = v
        return out
    v = json.loads(payload, object_pairs_hook=pairs)
    if type(v) is not dict or type(v.get('schema')) is not int or v['schema'] != 1:
        raise ValueError('Invalid signing schema')
    if kind == 'update':
        expected = f'https://github.com/{REPOSITORY}/releases/download/v{version}/LazyBuddy-windows-x86_64.zip'
        if (set(v) - {'sequence'} != {'schema','target','version','build','url','bytes','sha256','host_sha256'}
            or ('sequence' in v and (type(v['sequence']) is not int or not 0 < v['sequence'] < 2**64))
            or v['version'] != version or v['url'] != expected or v['target'] != 'windows-x86_64'
            or not all(digest(v[k]) for k in ('build','sha256','host_sha256'))
            or type(v['bytes']) is not int or not 0 < v['bytes'] <= 256*1024*1024):
            raise ValueError('Invalid application update payload')
    elif kind == 'catalog':
        if set(v) != {'schema','packages'} or not isinstance(v['packages'],list) or not 0 < len(v['packages']) <= 64:
            raise ValueError('Invalid module catalog')
        entries = {}
        for row in v['packages']:
            if (not isinstance(row,dict) or row.get('id') not in {'news','hongguo','reader','bilibili','python-runtime'}
                or row['id'] in entries or row.get('target') != 'windows-x86_64'
                or not package_version(row.get('version'))
                or not github_asset(row.get('url','')) or not digest(row.get('sha256'))
                or type(row.get('bytes')) is not int or not 0 < row['bytes'] <= 128*1024*1024
                or not isinstance(row.get('dependencies'),list)):
                raise ValueError('Invalid module package')
            entries[row['id']] = row
        for row in entries.values():
            for dep in row['dependencies']:
                if not isinstance(dep,dict) or set(dep) != {'id','version'} or entries.get(dep['id'],{}).get('version') != dep['version']:
                    raise ValueError('Missing exact module dependency')
    else:raise ValueError('Unknown signing payload')


def sign(key, catalog, update, output, root=ROOT):
    verify_identity(key,trusted_public(root))
    version = tomllib.loads((root/'apps/desktop/src-tauri/Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    planned = []
    for kind,path,name in [('catalog',catalog,'catalog.json'),('update',update,'app-update.json')]:
        if path is not None:
            payload = path.read_bytes();validate(payload,kind,version)
            planned.append((name,payload,key.sign(payload)))
    if not planned:raise ValueError('At least one public payload is required')
    output.mkdir(parents=True,exist_ok=False)
    for name,payload,signature in planned:
        key.public_key().verify(signature,payload)
        (output/name).write_bytes(payload);(output/(name+'.sig')).write_bytes(signature)


def remote_public(gh):
    keys = []
    for name in ('module-catalog.json','app-update-source.json'):
        result = subprocess.run([str(gh),'api',f'repos/{REPOSITORY}/contents/{name}?ref=main','--jq','.content'],capture_output=True)
        if result.returncode:raise RuntimeError('Cannot verify the current remote trust root')
        keys.append(bytes(json.loads(base64.b64decode(result.stdout))['public_key']))
    if keys[0] != keys[1] or len(keys[0]) != 32:raise ValueError('Remote release trust roots differ')
    return keys[0]


def register(key, public, gh):
    verify_identity(key,public)
    # gh encrypts stdin for the environment Secret; no plaintext file or command argument.
    seed = base64.b64encode(key.private_bytes_raw())
    result = subprocess.run([str(gh),'secret','set',KEY_ENV,'--repo',REPOSITORY,'--env',ENVIRONMENT],input=seed,capture_output=True)
    if result.returncode:raise RuntimeError('GitHub Secret registration failed; key value was not logged')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest='action',required=True)
    local = actions.add_parser('register',help='Run once on the PC holding the CURRENT DPAPI key')
    local.add_argument('--protected-key',type=Path,required=True)
    local.add_argument('--gh',type=Path)
    remote = actions.add_parser('sign',help='Run in the protected GitHub signing environment')
    remote.add_argument('--catalog',type=Path);remote.add_argument('--update',type=Path)
    remote.add_argument('--output',type=Path,required=True)
    args = parser.parse_args()
    if args.action == 'register':
        from windows_signing_key import load_key
        gh = args.gh or shutil.which('gh') or ROOT/'target/tools/github-cli/bin/gh.exe'
        public = trusted_public()
        if public != remote_public(gh):raise ValueError('Local trust root is stale; update this checkout before registering')
        register(load_key(args.protected_key),public,gh)
        print('Current release identity registered in the protected signing environment')
    else:
        try:key = Ed25519PrivateKey.from_private_bytes(base64.b64decode(os.environ.pop(KEY_ENV),validate=True))
        except (KeyError,ValueError):raise ValueError('Signing Secret is missing or invalid') from None
        sign(key,args.catalog,args.update,args.output)
        print('Public payloads signed; the private key is not an output')


if __name__ == '__main__':main()

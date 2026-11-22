#!/usr/bin/env python3
"""Sign an already verified Windows NSIS build and prepare the GRIT portal release."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('installer', type=Path)
parser.add_argument('destination', type=Path)
parser.add_argument('--notes', required=True)
parser.add_argument('--portable', type=Path, help='Verified Windows x64 portable ZIP from the same CI run')
parser.add_argument('--key', type=Path, default=Path.home() / '.local/share/grit-release-keys/grit-work.key')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
config = json.loads((root / 'src-tauri/tauri.conf.json').read_text())
version = config['version']
import re
if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
    raise SystemExit('Portal releases require a stable version.')
if args.installer.suffix.lower() != '.exe' or args.installer.read_bytes()[:2] != b'MZ':
    raise SystemExit('Expected a verified Windows NSIS installer.')
if args.key.with_suffix(args.key.suffix + '.pub').read_text().strip() != config['plugins']['updater']['pubkey']:
    raise SystemExit('Signing key does not match the public key embedded in the app.')
args.destination.mkdir(parents=True, exist_ok=False)
name = f'GRIT-Work_{version}_x64-setup.exe'
installer = args.destination.resolve() / name
shutil.copyfile(args.installer, installer)
env = dict(os.environ, TAURI_SIGNING_PRIVATE_KEY_PASSWORD='')
subprocess.run(['npm', 'run', 'tauri', '--', 'signer', 'sign', '--private-key-path', str(args.key), str(installer)], cwd=root, env=env, check=True, stdout=subprocess.DEVNULL)
signature_file = installer.with_suffix('.exe.sig')
signature = signature_file.read_text().strip()
# Keep detached signature alongside, but outside the uploader's strict release directory.
shutil.move(signature_file, args.destination.parent / (name + '.sig'))
manifest = {
    'schema_version': 1, 'id': 'grit-work', 'name': 'GRIT Work',
    'description': 'Lokalt arbetsunderlag och granskade tidrapporter för Spiris Tid.',
    'version': version, 'changelog': args.notes,
    'files': [{'name': name, 'platform': 'windows', 'arch': 'x64', 'size': installer.stat().st_size,
               'sha256': hashlib.file_digest(installer.open('rb'), 'sha256').hexdigest(),
               'updater_signature': signature}],
}
if args.portable:
    import zipfile
    with zipfile.ZipFile(args.portable) as archive:
        if archive.testzip() is not None or not any(n.endswith('grit-work.exe') for n in archive.namelist()):
            raise SystemExit('Portable ZIP is invalid or missing grit-work.exe.')
    portable = args.destination / f'GRIT-Work_{version}_portable_x64.zip'
    shutil.copyfile(args.portable, portable)
    with portable.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    manifest['files'].append({'name': portable.name, 'platform': 'windows', 'arch': 'x64',
                              'size': portable.stat().st_size, 'sha256': digest})
(args.destination / 'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')
print('Signed release prepared:', args.destination.resolve())
print('Validate and publish with login-gritprojects/scripts/software-release.py.')

#!/usr/bin/env python3
"""Publish a GitHub release only after verifying the complete six-target artifact set."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu', 'x86_64-pc-windows-msvc',
           'aarch64-pc-windows-msvc', 'x86_64-apple-darwin', 'aarch64-apple-darwin']
REPO = 'diocata/remnary'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    if args.tag != 'v' + version:
        raise SystemExit('Tag does not match Cargo.toml version.')
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    assets, checksums = [], []
    for target in TARGETS:
        name = f'remnary-{args.tag}-{target}'
        archive = args.directory / (name + ('.zip' if 'windows' in target else '.tar.gz'))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        checksum = archive.with_name(archive.name + '.sha256')
        if checksum.read_text().split() != [digest, archive.name]:
            raise SystemExit('Archive checksum mismatch: ' + target)
        if 'windows' in target:
            with zipfile.ZipFile(archive) as bundle:
                metadata = json.loads(bundle.read(name + '/BUILD.json'))
                binary = bundle.read(name + '/remnary.exe')
        else:
            with tarfile.open(archive) as bundle:
                metadata = json.load(bundle.extractfile(name + '/BUILD.json'))
                binary = bundle.extractfile(name + '/remnary').read()
        if not (metadata['commit'] == commit and metadata['target'] == target and
                metadata['version'] == version and metadata['release_smoke_passed'] and
                metadata['binary_sha256'] == hashlib.sha256(binary).hexdigest()):
            raise SystemExit('Build identity or runtime verification mismatch: ' + target)
        assets += [archive, checksum]
        checksums.append(f'{digest}  {archive.name}')
    combined = args.directory / 'SHA256SUMS'
    combined.write_text('\n'.join(checksums) + '\n')
    assets.append(combined)
    print('All six targets and executable/archive identities verified.')
    if args.verify_only:
        return
    subprocess.run(['git', 'rev-parse', '--verify', 'refs/tags/' + args.tag], check=True, capture_output=True)
    notes = ROOT / 'docs/releases' / (args.tag + '.md')
    subprocess.run(['gh', 'release', 'create', args.tag, '--repo', REPO, '--draft', '--verify-tag',
                    '--title', 'Remnary ' + args.tag, '--notes-file', str(notes)], check=True)
    subprocess.run(['gh', 'release', 'upload', args.tag, '--repo', REPO, *map(str, assets)], check=True)
    subprocess.run(['gh', 'release', 'edit', args.tag, '--repo', REPO, '--draft=false', '--latest'], check=True)


if __name__ == '__main__':
    main()

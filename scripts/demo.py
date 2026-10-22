#!/usr/bin/env python3
"""Create an isolated encrypted Remnary demo using only the bundled fictional history."""
import argparse
import os
from pathlib import Path
import shlex
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', default='remnary', help='installed command or path to a built CLI')
    parser.add_argument('--directory', type=Path, default=ROOT / '.remnary/demo',
                        help='new private directory; an existing directory is never overwritten')
    args = parser.parse_args()
    binary = shutil.which(args.binary)
    if binary is None:
        parser.error('build or install Remnary first, then provide --binary if needed')
    work = args.directory.expanduser().absolute()
    # Create parents privately as well; existing parents retain their permissions.
    previous_umask = os.umask(0o077)
    try:
        try:
            work.mkdir(mode=0o700, parents=True, exist_ok=False)
        except FileExistsError:
            parser.error("demo directory already exists; choose a new --directory")
    finally:
        os.umask(previous_umask)
    key, archive, vault = work / 'recovery.key', work / 'archive', work / 'vault'
    subprocess.run([binary, 'key', 'create', str(key)], check=True)
    command = [binary, '--data-dir', str(archive), '--key-file', str(key)]
    for operation in [('init',), ('import', str(ROOT / 'examples/demo/conversations.json')),
                      ('analyze',), ('discover',), ('vault', 'sync', str(vault), '--timezone', 'UTC'),
                      ('doctor',)]:
        subprocess.run([*command, *operation], check=True)
    print('\nFictional data only. No source discovery, real histories or model calls.')
    print('Open the demo dashboard:\n' + shlex.join([*command, 'web']))
    print('Markdown vault (plaintext): ' + str(vault))


if __name__ == '__main__':
    main()

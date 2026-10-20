#!/usr/bin/env python3
"""Kill synthetic backup/restore/conversion processes during observed writes.

Uses only a temporary synthetic archive, never real histories. This is an
optional manual integration check, not a simulation of hardware power loss.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/observr'))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    results = []
    with tempfile.TemporaryDirectory(prefix='observr-synthetic-recovery-') as work:
        root = Path(work)
        data, key, source = root/'data', root/'key', root/'conversations.json'
        records = []
        for conversation in range(4):
            mapping = {}
            for index in range(1024):
                identity = str(index)
                mapping[identity] = {'id': identity, 'parent': str(index-1) if index else None,
                                     'children': [str(index+1)] if index < 1023 else [],
                                     'message': {'id': identity, 'author': {'role': 'user'},
                                                 'content': {'content_type': 'text', 'parts': ['Synthetic recoveryneedle. '+('recovery context '*64)]}}}
            records.append({'id': f'recovery-{conversation}', 'mapping': mapping})
        source.write_text(json.dumps(records))
        del records

        def command(arguments, directory=data, plaintext=False):
            return [str(binary), '--data-dir', str(directory)]+(['--insecure-plaintext'] if plaintext else ['--key-file', str(key)])+list(map(str, arguments))

        def run(arguments, directory=data, plaintext=False):
            result = subprocess.run(command(arguments, directory, plaintext), capture_output=True, text=True, timeout=120)
            if result.returncode:
                raise RuntimeError(result.stderr)
            return result.stdout

        subprocess.run([str(binary), 'key', 'create', str(key)], check=True, capture_output=True)
        run(['init'])
        run(['import', source])

        def checksum(path):
            with path.open('rb') as file:
                return hashlib.file_digest(file, 'sha256').hexdigest()

        def health(directory):
            report = json.loads(run(['doctor'], directory))
            assert report['database_integrity'] and report['foreign_keys_valid']
            assert 'recoveryneedle' in run(['search', 'recoveryneedle', '--limit', '1'], directory)

        def interrupted(stage, arguments, watch, output, directory=data, plaintext=False):
            watch.mkdir(parents=True, exist_ok=True)
            with (root/f'{stage}.log').open('wb') as log:
                process = subprocess.Popen(command(arguments, directory, plaintext), stdout=log, stderr=log)
                observed = None
                deadline = time.monotonic()+30
                try:
                    while time.monotonic() < deadline and process.poll() is None:
                        for candidate in watch.glob('.tmp*'):
                            if candidate.is_file() and not candidate.name.endswith('-journal') and candidate.stat().st_size >= 8192:
                                observed = candidate
                                process.kill()
                                process.wait(timeout=10)
                                break
                        if observed:
                            break
                        time.sleep(.001)
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=10)
                assert observed is not None, f'{stage}: no live stage observed; interruption coverage unproven'
                assert process.returncode != 0, f'{stage}: process completed before kill; coverage unproven'
            assert not output.exists(), f'{stage}: incomplete destination was published'
            with observed.open('rb') as stage_file:
                assert stage_file.read(16) != b'SQLite format 3\0', f'{stage}: encrypted stage has a plaintext header'
            assert observed.stat().st_mode & 0o077 == 0, f'{stage}: stage is not private'
            results.append({'stage': stage, 'observed_live_write': True, 'destination_absent_after_kill': True,
                            'encrypted_private_stage_retained': True})

        original = checksum(data/'observr.sqlite3')
        backups = root/'backups'
        backup = backups/'archive.sqlite3'
        interrupted('backup', ['backup', backup], backups, backup)
        assert checksum(data/'observr.sqlite3') == original
        health(data)
        run(['backup', backup])
        restored = root/'restored'
        interrupted('restore', ['restore', backup], restored, restored/'observr.sqlite3', restored)
        assert checksum(data/'observr.sqlite3') == original
        run(['restore', backup], restored)
        health(restored)

        legacy = root/'plaintext'
        run(['init'], legacy, True)
        run(['import', source], legacy, True)
        legacy_hash = checksum(legacy/'observr.sqlite3')
        converted = root/'conversion'/'converted.sqlite3'
        interrupted('encrypt_copy', ['encrypt-copy', converted, '--new-key-file', key], converted.parent, converted, legacy, True)
        assert checksum(legacy/'observr.sqlite3') == legacy_hash
        run(['encrypt-copy', converted, '--new-key-file', key], legacy, True)
        final = root/'converted-restored'
        run(['restore', converted], final)
        health(final)
        assert checksum(legacy/'observr.sqlite3') == legacy_hash
        print(json.dumps({'synthetic': True, 'originals_preserved': True, 'retry_and_integrity_passed': True,
                          'checks': results, 'limits': 'SIGKILL during observed writes; not hardware power loss or every publication boundary'}, indent=2))


if __name__ == '__main__':
    main()

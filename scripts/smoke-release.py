#!/usr/bin/env python3
"""Exercise an actual release executable using a temporary, fictional encrypted archive."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import queue
from urllib.request import Request, urlopen
from urllib.error import HTTPError

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    binary = str(args.binary.resolve())
    with tempfile.TemporaryDirectory(prefix='remnary-release-smoke-') as temporary:
        work = Path(temporary) / 'Archive Δ with spaces'
        work.mkdir()
        key, archive, vault = work / 'recovery.key', work / 'archive', work / 'vault'
        def raw(*arguments):
            return subprocess.check_output([binary, *map(str, arguments)], text=True, encoding='utf-8', timeout=90)
        raw('--version')
        raw('key', 'create', key)
        base = ['--data-dir', str(archive), '--key-file', str(key)]
        def run(*arguments):
            return raw(*base, *arguments)
        run('init')
        fixture = ROOT / 'examples/demo/conversations.json'
        assert '3 new' in run('import', fixture)
        assert '3 unchanged' in run('import', fixture)
        run('analyze')
        assert 'No pending' in run('analyze')
        findings = json.loads(run('discover', '--all', '--limit', '100', '--json'))
        assert len(findings) == 9
        assert json.loads(run('profile', '--json')) == []
        record = next(r for r in findings if 'prefer Rust for local tools' in r['observation']['summary'])
        run('feedback', record['id'], 'confirm')
        assert len(json.loads(run('profile', '--json'))) == 1
        run('vault', 'sync', vault)
        note = vault / 'Profile.md'
        note.write_text(note.read_text(encoding='utf-8') + '\nMy own annotation.\n', encoding='utf-8')
        run('vault', 'sync', vault)
        assert note.read_text(encoding='utf-8').endswith('My own annotation.\n')
        assert 'SQLite' in run('search', 'SQLite')
        health = json.loads(run('doctor'))
        assert health['database_integrity'] and health['foreign_keys_valid']
        assert (archive / 'remnary.sqlite3').read_bytes()[:16] != b'SQLite format 3\x00'
        backup, restored = work / 'backup.sqlite3', work / 'restored'
        run('backup', backup)
        raw('--data-dir', restored, '--key-file', key, 'restore', backup)
        assert json.loads(raw('--data-dir', restored, '--key-file', key, 'doctor'))['database_integrity']
        assert len(json.loads(raw('--data-dir', restored, '--key-file', key, 'profile', '--json'))) == 1
        server = subprocess.Popen([binary, *base, 'web', '--port', '0'], stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True, encoding='utf-8')
        lines = queue.Queue()
        def read_lines():
            for line in server.stdout:
                lines.put(line)
        threading.Thread(target=read_lines, daemon=True).start()
        try:
            endpoint = None
            for _ in range(10):
                line = lines.get(timeout=15)
                match = re.search(r'(http://127\.0\.0\.1:\d+)/#token=([a-f0-9]+)', line)
                if match:
                    endpoint, token = match.groups()
                    break
            assert endpoint, 'dashboard did not start'
            with urlopen(endpoint, timeout=10) as response:
                assert b'Remnary' in response.read()
            request = Request(endpoint + '/api/status', headers={'Authorization': 'Bearer ' + token})
            with urlopen(request, timeout=10) as response:
                assert response.status == 200
            try:
                urlopen(endpoint + '/api/status', timeout=10)
                raise AssertionError('unauthenticated API request succeeded')
            except HTTPError as error:
                assert error.code == 401
        finally:
            server.terminate()
            server.wait(timeout=15)
            server.stdout.close()
        print(json.dumps({'release_smoke': 'passed', 'encrypted': True, 'backup_restore': True,
                          'review_vault_search': True, 'unicode_paths': True,
                          'dashboard_and_auth': True, 'crypto_provider': health['cipher_provider']}))


if __name__ == '__main__':
    main()

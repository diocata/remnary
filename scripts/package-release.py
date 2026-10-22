#!/usr/bin/env python3
"""Check, build and package a native Windows, Linux or macOS executable. Does not publish."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {'aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu',
           'aarch64-unknown-linux-gnu', 'x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc'}


def run(*arguments, **kwargs):
    return subprocess.run(list(map(str, arguments)), cwd=ROOT, check=True, **kwargs)


def output(*arguments, **kwargs):
    return run(*arguments, capture_output=True, text=True, **kwargs).stdout.strip()


def clean_commit():
    if output('git', 'status', '--porcelain', '--untracked-files=normal'):
        raise SystemExit('Commit working-tree changes before packaging.')
    return output('git', 'rev-parse', 'HEAD')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', action='store_true', help='create a versioned distribution archive')
    parser.add_argument('--target', choices=sorted(TARGETS), help='native target; defaults to rustc host')
    args = parser.parse_args()
    env = os.environ.copy()
    isolated = ROOT / '.tools/cargo/bin/cargo'
    cargo = str(isolated) if isolated.exists() else 'cargo'
    rustc = str(isolated.with_name('rustc')) if isolated.exists() else 'rustc'
    if isolated.exists():
        env['CARGO_HOME'], env['RUSTUP_HOME'] = str(ROOT / '.tools/cargo'), str(ROOT / '.tools/rustup')
    version_info = output(rustc, '-vV', env=env)
    host = next(line.removeprefix('host: ') for line in version_info.splitlines() if line.startswith('host: '))
    target = args.target or host
    if target not in TARGETS:
        raise SystemExit('This target has no release recipe.')
    expected = 'Windows' if 'windows' in target else 'Darwin' if 'apple' in target else 'Linux'
    if platform.system() != expected:
        raise SystemExit('Build and exercise each package on its own operating system.')
    commit = clean_commit()
    epoch = int(output('git', 'show', '-s', '--format=%ct', 'HEAD'))
    package = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']
    version = package['version']
    if env.get('RUSTFLAGS') or env.get('CARGO_ENCODED_RUSTFLAGS'):
        raise SystemExit('Unset custom Rust flags before packaging.')
    if expected == 'Darwin':
        env['MACOSX_DEPLOYMENT_TARGET'] = '13.0'
    run(cargo, 'fmt', '--check', env=env)
    run(cargo, 'test', '--locked', '--all-targets', '--target', target, env=env)
    run(cargo, 'clippy', '--locked', '--all-targets', '--target', target, '--', '-D', 'warnings', env=env)
    flags = [f'--remap-path-prefix={path}=/remnary/build/{index}' for index, path in
             enumerate([ROOT, ROOT / '.tools/cargo', Path.home() / '.cargo'])]
    if expected == 'Windows':
        flags += ['-C', 'target-feature=+crt-static']
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    run(cargo, 'build', '--release', '--locked', '--target', target,
        '--target-dir', 'target/package-build', env=env)
    executable = 'remnary.exe' if expected == 'Windows' else 'remnary'
    binary = ROOT / 'target/package-build' / target / 'release' / executable
    signing = 'unsigned'
    if expected == 'Darwin':
        run('codesign', '--force', '--sign', '-', binary)
        run('codesign', '--verify', '--strict', binary)
        signing = 'ad-hoc'
    run(sys.executable, ROOT / 'scripts/smoke-release.py', '--binary', binary)
    if clean_commit() != commit:
        raise SystemExit('Source changed during the build.')
    name = f'remnary-v{version}{"" if args.release else "-evaluation-" + commit[:12]}-{target}'
    destination = ROOT / 'target/dist'
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / (name + ('.zip' if expected == 'Windows' else '.tar.gz'))
    if archive.exists():
        raise SystemExit(f'Package already exists: {archive}')
    metadata = {'version': version, 'commit': commit, 'target': target, 'license': package['license'],
                'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                'cargo_lock_sha256': hashlib.sha256((ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
                'rustc': version_info, 'platform_tested': platform.platform(),
                'code_signing': signing, 'developer_id_signed': False, 'notarized': False,
                'minimum_macos': '13.0' if expected == 'Darwin' else None,
                'linux_libc': 'glibc; release CI builds on Ubuntu 22.04' if expected == 'Linux' else None,
                'static_msvc_runtime': expected == 'Windows', 'release_smoke_passed': True}
    files = [(executable, binary)]
    selected = ['README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'SECURITY.md',
                'docs', 'examples/demo', 'scripts/demo.py', 'scripts/render-doc-art.py', 'scripts/render-doc-media.py']
    for entry in selected:
        path = ROOT / entry
        files.extend((file.relative_to(ROOT).as_posix(), file) for file in
                     (sorted(path.rglob('*')) if path.is_dir() else [path]) if file.is_file())
    # Preserve common native notices, overlay a freshly resolved inventory for this target.
    for entry in ['README.md', 'notices']:
        path = ROOT / 'third_party' / entry
        files.extend((file.relative_to(ROOT).as_posix(), file) for file in
                     (sorted(path.rglob('*')) if path.is_dir() else [path]) if file.is_file())
    with tempfile.TemporaryDirectory(prefix='remnary-target-notices-') as temporary:
        notices = Path(temporary)
        run(sys.executable, ROOT / 'scripts/dependency-notices.py', '--target', target, '--output', notices)
        files.extend(('third_party/' + file.relative_to(notices).as_posix(), file)
                     for file in sorted(notices.rglob('*')) if file.is_file())
        entries = [(relative, path.read_bytes(), 0o755 if relative == executable else 0o644)
                   for relative, path in files]
        entries.append(('BUILD.json', (json.dumps(metadata, indent=2)+'\n').encode(), 0o644))
        with tempfile.NamedTemporaryFile(dir=destination, prefix='.package-', delete=False) as handle:
            stage = Path(handle.name)
        try:
            if expected == 'Windows':
                with zipfile.ZipFile(stage, 'w', compression=zipfile.ZIP_DEFLATED) as bundle:
                    for relative, data, mode in sorted(entries):
                        info = zipfile.ZipInfo(f'{name}/{relative}')
                        info.external_attr = mode << 16
                        info.compress_type = zipfile.ZIP_DEFLATED
                        bundle.writestr(info, data)
            else:
                with stage.open('wb') as handle:
                    with gzip.GzipFile(fileobj=handle, mode='wb', filename='', mtime=0) as compressed:
                        with tarfile.open(fileobj=compressed, mode='w') as bundle:
                            for relative, data, mode in sorted(entries):
                                info = tarfile.TarInfo(f'{name}/{relative}')
                                info.size, info.mode, info.mtime = len(data), mode, epoch
                                bundle.addfile(info, io.BytesIO(data))
            with stage.open('r+b') as handle:
                os.fsync(handle.fileno())
            os.link(stage, archive)
        finally:
            stage.unlink(missing_ok=True)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    with archive.with_name(archive.name+'.sha256').open('x') as handle:
        handle.write(f'{digest}  {archive.name}\n')
    print(f'Checked package: {archive}\nSHA-256: {digest}')


if __name__ == '__main__':
    main()

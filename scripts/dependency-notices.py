#!/usr/bin/env python3
"""Collect locked target dependency notices and required source into a chosen output tree."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def main():
    env = os.environ.copy()
    isolated = ROOT / '.tools/cargo/bin/cargo'
    if isolated.exists():
        env['CARGO_HOME'] = str(ROOT / '.tools/cargo')
        env['RUSTUP_HOME'] = str(ROOT / '.tools/rustup')
    cargo = str(isolated) if isolated.exists() else 'cargo'
    rustc = str(isolated.with_name('rustc')) if isolated.exists() else 'rustc'
    version = subprocess.check_output([rustc, '-vV'], text=True, env=env)
    host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default=host)
    parser.add_argument('--output', type=Path, default=ROOT / 'third_party')
    args = parser.parse_args()
    output = args.output.absolute()
    if output.is_symlink():
        raise RuntimeError('refusing symlink notice output')
    output.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output([cargo, 'metadata', '--locked', '--format-version', '1',
                                                 '--filter-platform', args.target], cwd=ROOT, env=env))
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    selected, pending = set(), [metadata['resolve']['root']]
    while pending:
        identity = pending.pop()
        if identity in selected:
            continue
        selected.add(identity)
        pending.extend(dep['pkg'] for dep in nodes[identity]['deps'])
    destination = output / 'rust-notices'
    if destination.is_symlink():
        raise RuntimeError('refusing symlink notice output')
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    manifest, missing = [], []
    for package in sorted((packages[i] for i in selected), key=lambda p: (p['name'], p['version'])):
        if package['source'] is None:
            continue
        base = Path(package['manifest_path']).parent
        out = destination / f'{package["name"]}-{package["version"]}'
        files = [p for p in base.iterdir() if p.is_file() and p.name.upper().startswith(
            ('LICENSE', 'COPYING', 'COPYRIGHT', 'NOTICE', 'AUTHORS'))]
        if package.get('license_file'):
            files.append(base / package['license_file'])
        # The wrapper crate's license does not replace bundled OpenSSL's notices.
        if package['name'] == 'openssl-src':
            files.extend(p for p in (base / 'openssl').iterdir() if p.is_file() and
                         p.name.upper().startswith(('LICENSE', 'COPYING', 'COPYRIGHT', 'NOTICE', 'AUTHORS')))
        copied = []
        for source in sorted(set(files)):
            if source.stat().st_size > 4 * 1024 * 1024:
                raise RuntimeError(f'oversized notice for {package["name"]}')
            relative = source.relative_to(base)
            target = out / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            copied.append(str(target.relative_to(output)).replace('\\', '/'))
        if not copied:
            for candidate in base.parent.glob('LICENSE*'):
                if candidate.is_file():
                    out.mkdir(exist_ok=True)
                    target = out / candidate.name
                    shutil.copyfile(candidate, target)
                    copied.append(str(target.relative_to(output)).replace('\\', '/'))
        if not copied:
            missing.append(package['name'])
        manifest.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'),
                         'source': package['source'], 'repository': package.get('repository'), 'notices': sorted(copied)})
    for package in (p for p in manifest if p['license'] and 'MPL-' in p['license']):
        cache = Path(env.get('CARGO_HOME', str(Path.home() / '.cargo'))) / 'registry/cache'
        candidates = list(cache.glob(f'*/{package["name"]}-{package["version"]}.crate'))
        if len(candidates) != 1:
            raise RuntimeError('MPL source archive missing or ambiguous')
        target = output / 'sources' / candidates[0].name
        target.parent.mkdir(exist_ok=True)
        shutil.copyfile(candidates[0], target)
        package['source_archive'] = str(target.relative_to(output)).replace('\\', '/')
    sysroot = Path(subprocess.check_output([rustc, '--print', 'sysroot'], text=True, env=env).strip())
    documentation = sysroot / 'share/doc/rust'
    toolchain = output / 'rust-toolchain'
    toolchain.mkdir(parents=True, exist_ok=True)
    if not (documentation / 'COPYRIGHT-library.html').is_file():
        raise RuntimeError('Rust library notices missing; install the rust-docs toolchain component')
    shutil.copyfile(documentation / 'COPYRIGHT-library.html', toolchain / 'COPYRIGHT-library.html')
    shutil.copytree(documentation / 'licenses', toolchain / 'licenses', dirs_exist_ok=True)
    report = {'target': args.target, 'rustc': version.strip(), 'packages': manifest, 'missing_notice_files': missing}
    (output / 'dependencies.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f'{len(manifest)} dependency notice sets; {len(missing)} missing notice sets for {args.target}.')
    if missing:
        raise RuntimeError('notice inventory incomplete: ' + ', '.join(missing))


if __name__ == '__main__':
    main()

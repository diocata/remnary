#!/usr/bin/env python3
"""Collect notices from the locked dependency sources for one release target.

Does not read conversation archives. Run again after changing Cargo.lock or
Rust. Collection is an inventory, not a substitute for license review.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess


def main():
    root = Path(__file__).resolve().parent.parent
    env = os.environ.copy()
    isolated = root/'.tools/cargo/bin/rustc'
    if isolated.exists():
        env['CARGO_HOME'] = str(root/'.tools/cargo')
        env['RUSTUP_HOME'] = str(root/'.tools/rustup')
    rustc = str(isolated) if isolated.exists() else 'rustc'
    version = subprocess.check_output([rustc, '-vV'], text=True, env=env)
    host = next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default=host)
    args = parser.parse_args()
    metadata = json.loads(subprocess.check_output([str(root/'scripts/cargo-local'), 'metadata', '--locked', '--format-version', '1', '--filter-platform', args.target], cwd=root))
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    selected, pending = set(), [metadata['resolve']['root']]
    while pending:
        identity = pending.pop()
        if identity in selected:
            continue
        selected.add(identity)
        pending.extend(dep['pkg'] for dep in nodes[identity]['deps'])
    destination = root/'third_party/rust-notices'
    # Rebuild only this script-owned output tree; never visit archive locations.
    if destination.is_symlink():
        raise RuntimeError('refusing symlink notice output')
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    manifest, missing = [], []
    for package in sorted((packages[i] for i in selected if i != metadata['resolve']['root']), key=lambda p: (p['name'], p['version'])):
        base = Path(package['manifest_path']).parent
        out = destination/f'{package["name"]}-{package["version"]}'
        files = [p for p in base.rglob('*') if p.is_file() and not p.is_symlink() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE', 'COPYRIGHT'))]
        if package.get('license_file'):
            candidate = base/package['license_file']
            if candidate.is_file() and candidate not in files:
                files.append(candidate)
        copied = []
        for source in files:
            relative = source.relative_to(base)
            if source.stat().st_size > 4*1024*1024:
                raise RuntimeError(f'oversized notice for {package["name"]}')
            target = out/relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            copied.append(str(target.relative_to(root/'third_party')))
        if not copied and package['source'].startswith('git+'):
            # Workspace crates can share a repository-root license.
            for candidate in base.parent.glob('LICENSE*'):
                if candidate.is_file():
                    out.mkdir(exist_ok=True)
                    target = out/candidate.name
                    shutil.copyfile(candidate, target)
                    copied.append(str(target.relative_to(root/'third_party')))
        if not copied:
            missing.append(package['name'])
        manifest.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'),
                         'source': package['source'], 'repository': package.get('repository'), 'notices': sorted(copied)})
    # Include the unmodified MPL-covered crate's source form in the distribution.
    mpl = [p for p in manifest if p['license'] and 'MPL-' in p['license']]
    for package in mpl:
        cache = Path(env.get('CARGO_HOME', Path.home()/'.cargo'))/'registry/cache'
        candidates = list(cache.glob(f'*/{package["name"]}-{package["version"]}.crate'))
        if len(candidates) != 1:
            raise RuntimeError('MPL source archive missing or ambiguous')
        out = root/'third_party/sources'
        out.mkdir(exist_ok=True)
        target = out/candidates[0].name
        shutil.copyfile(candidates[0], target)
        package['source_archive'] = str(target.relative_to(root/'third_party'))
    sysroot = Path(subprocess.check_output([rustc, '--print', 'sysroot'], text=True, env=env).strip())
    documentation = sysroot/'share/doc/rust'
    toolchain = root/'third_party/rust-toolchain'
    toolchain.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(documentation/'COPYRIGHT-library.html', toolchain/'COPYRIGHT-library.html')
    shutil.copytree(documentation/'licenses', toolchain/'licenses', dirs_exist_ok=True)
    report = {'target': args.target, 'rustc': version.strip(), 'packages': manifest, 'missing_notice_files': missing}
    (root/'third_party/dependencies.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    print(f'{len(manifest)} dependency notice sets; Rust library notices; {len(mpl)} MPL source archive(s); {len(missing)} missing notice sets.')
    if missing:
        raise RuntimeError('notice inventory incomplete: '+', '.join(missing))


if __name__ == '__main__':
    main()

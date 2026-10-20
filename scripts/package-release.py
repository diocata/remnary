#!/usr/bin/env python3
"""Build a local, unpublished Apple Silicon evaluation package from a clean commit."""
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def run(*arguments, **kwargs):
    return subprocess.run(arguments, cwd=ROOT, check=True, **kwargs)


def output(*arguments):
    return run(*arguments, capture_output=True, text=True).stdout.strip()


def clean_commit():
    if output("git", "status", "--porcelain", "--untracked-files=normal"):
        raise SystemExit("Commit or remove working-tree changes before packaging.")
    return output("git", "rev-parse", "HEAD")


def main():
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise SystemExit("This package recipe currently supports Apple Silicon macOS only.")
    commit = clean_commit()
    epoch = int(output("git", "show", "-s", "--format=%ct", "HEAD"))
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    target = "aarch64-apple-darwin"
    cargo = str(ROOT / "scripts/cargo-local")
    run(cargo, "fmt", "--check")
    run(cargo, "test", "--locked", "--all-targets")
    run(cargo, "clippy", "--locked", "--all-targets", "--", "-D", "warnings")
    env = os.environ.copy()
    # Do not embed this workstation's source/cache paths in distributed binaries.
    # Encoded flags preserve paths containing spaces without shell expansion.
    if env.get("RUSTFLAGS") or env.get("CARGO_ENCODED_RUSTFLAGS"):
        raise SystemExit("Unset custom Rust flags before creating a release package.")
    prefixes = [ROOT, ROOT / ".tools/cargo", Path.home() / ".cargo"]
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(
        f"--remap-path-prefix={path}=/observr/build/{index}"
        for index, path in enumerate(prefixes)
    )
    run(cargo, "build", "--release", "--locked", "--target", target,
        "--target-dir", "target/package-build", env=env)
    binary = ROOT / "target/package-build" / target / "release/observr"
    run(str(binary), "--version")
    # Exercise the packaged binary against a fresh encrypted archive, not real data.
    with tempfile.TemporaryDirectory(prefix="observr-release-smoke-") as temporary:
        base = Path(temporary)
        key, archive = base / "recovery.key", base / "archive"
        run(str(binary), "key", "create", str(key))
        command = [str(binary), "--data-dir", str(archive), "--key-file", str(key)]
        for args in [("init",), ("import", str(ROOT / "tests/fixtures/chatgpt.json")),
                     ("analyze",), ("search", "Rust"), ("doctor",)]:
            run(*command, *args)
        if (archive / "observr.sqlite3").read_bytes().startswith(b"SQLite format 3"):
            raise SystemExit("Release smoke archive unexpectedly has a plaintext header.")
    if clean_commit() != commit:
        raise SystemExit("The source commit changed during the build; package not created.")
    name = f"observr-{version}-evaluation-{commit[:12]}-{target}"
    destination = ROOT / "target/dist"
    destination.mkdir(parents=True, exist_ok=True)
    package = destination / f"{name}.tar.gz"
    if package.exists():
        raise SystemExit(f"Package already exists: {package}")
    metadata = {
        "version": version, "commit": commit, "target": target,
        "status": "evaluation; production readiness not established",
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "minimum_macos": "27.0.1", "signed": False, "notarized": False,
    }
    files = [("observr", binary)]
    for selected in ["README.md", "LICENSE", "SECURITY.md", "docs", "third_party"]:
        path = ROOT / selected
        files.extend((str(file.relative_to(ROOT)), file) for file in
                     (sorted(path.rglob("*")) if path.is_dir() else [path]) if file.is_file())
    # Fixed archive metadata makes repeated packaging of identical bytes stable.
    # Native compiler/toolchain changes can still change binary bytes.
    staged = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination, prefix=".package-", delete=False) as temporary:
            staged = Path(temporary.name)
            with gzip.GzipFile(fileobj=temporary, mode="wb", filename="", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as bundle:
                    entries = [(name, path.read_bytes(), 0o755 if name == "observr" else 0o644)
                               for name, path in files]
                    entries.append(("BUILD.json", (json.dumps(metadata, indent=2)+"\n").encode(), 0o644))
                    for relative, data, mode in sorted(entries):
                        info = tarfile.TarInfo(f"{name}/{relative}")
                        info.size, info.mode, info.mtime = len(data), mode, epoch
                        bundle.addfile(info, io.BytesIO(data))
            temporary.flush()
            os.fsync(temporary.fileno())
        # Exclusive publication; never overwrite an existing package.
        os.link(staged, package)
        digest = hashlib.sha256(package.read_bytes()).hexdigest()
        checksum = package.with_name(package.name + ".sha256")
        with checksum.open("x") as handle:
            handle.write(f"{digest}  {package.name}\n")
        print(f"Local evaluation package: {package}\nSHA-256: {digest}")
    finally:
        if staged is not None:
            staged.unlink(missing_ok=True)


if __name__ == "__main__":
    main()

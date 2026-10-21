# Local evaluation packages

The initial package target is `aarch64-apple-darwin`. This recipe creates an **evaluation build**, not a production sign-off. Current acceptance evidence and unresolved findings are in [readiness.md](readiness.md).

From a clean committed checkout with Python 3.11+, the pinned Rust toolchain and macOS C build tools:

```sh
python3 scripts/package-release.py
```

The script runs formatting, locked tests and Clippy, builds an optimized binary with local source/cache paths remapped, then exercises encrypted initialization, synthetic import, analysis, search and integrity checks using that exact binary. Source changes during the build stop publication of the package. The result goes into the ignored `target/dist` directory; no remote repository or release is created.

The archive includes the CLI, README, MIT license, security guidance, product documentation and complete target-specific dependency notices/source obligations. `BUILD.json` records the source commit, target, Cargo.lock hash and binary hash. An adjacent `.sha256` file records the archive checksum. Archive timestamps, ownership and ordering are fixed; this does not assert cross-machine reproducible native compilation.

Extract the archive into a directory you own and run `./observr --version` there. To verify the archive before extraction:

```sh
shasum -a 256 -c observr-VERSION-evaluation-COMMIT-aarch64-apple-darwin.tar.gz.sha256
```

The binary has the Apple Silicon linker’s ad-hoc signature, which the recipe verifies. It has no Developer ID distribution signature and is unnotarized. Public distribution, code signing, notarization, hosted CI evidence and a repository-specific private security reporting channel require separate release setup. This script performs no publication, signing, installation or model download.


## Local release after acceptance

After completing the applicable acceptance gates in [readiness.md](readiness.md), create the local release archive with:

```sh
python3 scripts/package-release.py --release
```

This uses the same clean-checkout, test, lint, optimized-build, signature and encrypted-smoke checks. The flag labels the archive `observr-VERSION-COMMIT-aarch64-apple-darwin.tar.gz` and records local-release status in `BUILD.json`; the linked acceptance evidence supplies the qualification. The default command continues to produce evaluation archives. Both modes remain local and unpublished, with the same signing/distribution limits above.

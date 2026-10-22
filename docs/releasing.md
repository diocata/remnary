# Native packages and GitHub Releases

## Build a checked local package

From a clean committed checkout with Rust 1.99.1, Python 3.11+, the `rust-docs` component for license inventory, and the platform's [native build prerequisites](platforms.md):

```sh
python scripts/package-release.py --release
```

On systems where Python is named `python3`, use that instead. `--target TARGET` selects one of the six supported triples on the same operating system; CI also supplies a native runner for its CPU. The default is the Rust compiler's host triple. No publication occurs in this script. Omitting `--release` adds an evaluation/commit suffix.

The recipe requires a clean source commit, runs formatting, locked all-target tests and Clippy, builds an optimized executable with local build/cache paths remapped, and exercises that exact executable. Windows/Linux use vendored OpenSSL for SQLCipher; Windows release binaries statically link the MSVC runtime. macOS uses CommonCrypto and an explicit ad-hoc signature. The recipe then collects the actual target's dependency notices, including OpenSSL's own bundled notices and required unmodified MPL source. The existing checked-in Mac inventory is not reused as a Windows/Linux inventory.

The runtime smoke check uses only a temporary fictional archive. It verifies Unicode/spaced paths, encrypted headers, import/reimport, cached rules analysis, review/profile, annotation-preserving vault writes, search, encrypted backup/restore, integrity, authenticated localhost HTTP and denial without a bearer token. It prints no key or dashboard token.

Outputs under ignored `target/dist/`:

```text
remnary-vVERSION-TARGET.zip          # Windows
remnary-vVERSION-TARGET.tar.gz       # Linux or macOS
ARCHIVE.sha256
```

Each archive includes `remnary`/`remnary.exe`, `BUILD.json`, README, both license texts, NOTICE, security guidance, documentation/images/GIFs, the fictional demo and rendering scripts, and freshly resolved target-specific third-party notices/source obligations. Python is required only for the optional demo/rendering scripts, not to run the binary or dashboard. System fonts and model weights are not bundled.

`BUILD.json` records version, source commit, target, toolchain, runner environment, Cargo.lock and binary hashes, crypto/runtime distribution policy, signing state and completed runtime smoke. The adjacent checksum identifies the whole archive. Archive metadata/order is fixed; this is not a cross-machine reproducibility claim. Existing packages are never overwritten by this script.

## Publish a version

Update Cargo.toml/Cargo.lock and `docs/releases/vVERSION.md`, push the source commit, and verify the ordinary matrix run. Create and push an annotated tag matching the package version:

```sh
git tag -a vVERSION -m "Remnary vVERSION"
git push origin vVERSION
```

The [release workflow](../.github/workflows/ci.yml) repeats the six native jobs and advisory scan. Build jobs have read-only repository permissions and upload only `target/dist` artifacts. A tag-only publisher has scoped content-write permission, downloads the six artifact sets and invokes `scripts/publish-release.py`.

The publisher verifies all six archive checksums, version/commit/target metadata, executable hashes and smoke receipts. A missing target, mismatched version/commit or bad checksum stops publication. It creates a draft, uploads all archives and checksum files plus combined `SHA256SUMS`, then publishes and marks the release latest. No build failure publishes a partial platform set. The script refuses to silently overwrite an existing release; inspect a failed draft before retrying.

Users download from [the latest release](https://github.com/diocata/remnary/releases/latest). SHA-256 files detect byte mismatch; they are not an independent signing identity. macOS remains ad-hoc signed/unnotarized, Windows remains unsigned, and no installer or auto-updater is implied.

## Historical packages

The earlier v0.1 recipe produced only local Apple Silicon evaluation/release archives with commit-prefixed names and a narrower macOS baseline. Hashes in the historical readiness/performance documents identify those binaries. v0.2 introduces the versioned six-platform archives, native runtime matrix and public tag-based publishing; it does not retroactively change those historical measurements.

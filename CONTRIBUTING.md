# Contributing

Production readiness remains open; consult [release evidence](docs/readiness.md) and preserve the original brief's evidence/privacy requirements.

Use the pinned toolchain and run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

`./scripts/cargo-local` uses this checkout's isolated development toolchain when present. Native SQLCipher compilation needs a C compiler; macOS uses CommonCrypto. The supported validation target is Apple Silicon macOS 27.0.1. See [platform policy](docs/platforms.md) and [native dependency baseline](docs/native-dependencies.md) before changing targets or dependency pins.

Use synthetic fixtures only. Do not commit account exports, session logs, database/backup files, model caches, keys, bearer tokens or screenshots of real histories. Keep private evaluation material under ignored `.observr/` or outside the repository. Review staged changes before committing.

Importer changes need stable identity, branch/role attribution, unknown-time, bounded-input, read-only and reimport evidence. Never turn provider instructions, tool responses, agent requests or pasted advice into confirmed personal facts. Model output is untrusted until typed and exact-evidence validation succeeds. Keep metadata and credentials behind the common redaction boundary.

Schema changes must migrate transactionally and preserve existing source revisions, user feedback and deletion tombstones. Filesystem changes must preserve unowned/edited content and user-written sections. Test failure/recovery paths where canonical transactions and generated files cross boundaries.

Prefer direct, small modules over new services and frameworks. Changes should explain the user-visible result and relevant validation. CI is configured, but a local successful run is not evidence that hosted CI has run. No remote is configured or public contribution/license policy finalized yet; do not publish private evaluation data as part of setup.

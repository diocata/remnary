# Contributing

Production readiness remains open; consult [release evidence](docs/readiness.md) and preserve the original brief's evidence/privacy requirements.

Use the pinned toolchain and run:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

`./scripts/cargo-local` uses this checkout's isolated development toolchain when present. Native SQLCipher compilation needs a C compiler; macOS uses CommonCrypto. The release matrix builds and runs native tests on Windows, Linux and macOS, on x64 and ARM64. Windows/Linux bundle OpenSSL; source builds need its native build prerequisites. See [platform policy](docs/platforms.md) and [native dependency baseline](docs/native-dependencies.md) before changing targets or dependency pins.

Use synthetic fixtures only. Do not commit account exports, session logs, database/backup files, model caches, keys, bearer tokens or screenshots of real histories. Keep private evaluation material under ignored `.remnary/` or outside the repository. Review staged changes before committing.

Importer changes need stable identity, branch/role attribution, unknown-time, bounded-input, read-only and reimport evidence. Never turn provider instructions, tool responses, agent requests or pasted advice into confirmed personal facts. Model output is untrusted until typed and exact-evidence validation succeeds. Keep metadata and credentials behind the common redaction boundary.

Schema changes must migrate transactionally and preserve existing source revisions, user feedback and deletion tombstones. Filesystem changes must preserve unowned/edited content and user-written sections. Test failure/recovery paths where canonical transactions and generated files cross boundaries.

Prefer direct, small modules over new services and frameworks. Changes should explain the user-visible result and relevant validation. CI is configured, but a local successful run is not evidence that hosted CI has run. Open focused pull requests at https://github.com/diocata/remnary with the behavior change and verification. Never publish private evaluation data.

Unless explicitly stated otherwise, original contributions intentionally submitted for inclusion are offered under the project’s MIT OR Apache-2.0 terms, without additional terms. Third-party material must retain its original attribution and compatible license. Documentation screenshots must use the fictional archive under `examples/demo`, never real histories. See [documentation media](docs/assets/README.md) for provenance and regeneration.

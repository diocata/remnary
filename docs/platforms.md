# Platform policy

The v0.1 release target is **Apple Silicon macOS**, with a minimum supported OS of **macOS 27.0.1**. The complete local workflow has been exercised on macOS 27.0.1, Apple M4 Pro, 24 GiB memory, using Rust 1.99.1. This narrow initial baseline avoids promising compatibility with OS versions and hardware that have not been exercised. Earlier macOS versions may compile/run but are outside the validated target.

A release build must use the pinned dependency lockfile, report SQLCipher 4.19/CommonCrypto through `doctor`, and pass tests, formatting, Clippy, installed-binary import/search/review/recovery checks and the synthetic interruption check. Native compilation needs Apple's C build tools. It does not require a database server, Homebrew OpenSSL, Perl or Ollama for ordinary archive/rules use. Local-model analysis additionally needs a separately installed compatible Ollama daemon/model and enough memory for that model/context.

Intel macOS, Linux, Windows and cross-compilation are unverified and excluded from the current release target. Linux source builds need suitable system crypto headers/libraries; no package or service installation is automated. Expand this policy only after running the full encrypted workflow and platform-specific credential-store/permission/recovery checks on the proposed target.

No application installer, signing/notarization claim or automatic updater is provided yet. CI configuration is present but a hosted run has not been observed. A successful source build alone is insufficient evidence for a distributable release.

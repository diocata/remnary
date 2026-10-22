# Changelog

## 0.1.0 — 2026-10-22

Initial local release for Apple Silicon macOS 27.0.1.

Expanded the project guide with original brand/workflow illustrations, actual synthetic dashboard screenshots, two guided GIFs, a command reference and reproducible fictional examples. Original Remnary code, documentation and artwork are now offered under MIT OR Apache-2.0; third-party licenses remain separate. Local packages include both license texts, NOTICE, documentation media and the demo/rendering scripts.

Renamed the project, Rust crate, CLI, dashboard, vault output, new archive/Keychain namespaces, examples and packaging to **Remnary**. The brand combines remnant and diary: a living record of your thoughts, memories, and experiences. **Your thoughts leave traces. We connect them.** Existing archive, partial-view and Markdown formats retain explicit compatibility readers; see [rename guidance](renaming.md).

- Rust CLI with an embedded SQLCipher archive, immutable source revisions, full-text search and private Keychain/recovery-key handling.
- Selected ChatGPT JSON/ZIP/shard imports, connected ChatGPT history views and local Claude Code, Codex and Kiro IDE adapters.
- Typed evidence-linked observations, comparison-request recurrence, related-topic views, confirmed-only profiles, decision events and daily activity digests.
- Optional separately installed local Ollama analysis with bounded input/output, redaction, constrained citations, semantic verification and conservative identity matching.
- Authenticated loopback dashboard and managed Obsidian-compatible Markdown with exact source links, preserved owner annotations and explicit feedback import.
- Persistent exclusions, review history, deletion safeguards, encrypted backup/restore, key rotation by verified copy and recovery diagnostics.

Validated with 223 offline tests (including five rename-compatibility regressions), separate unchanged model regressions, an 86-conversation real-history assessment and 2,000-conversation / 32,000-message performance and dashboard checks. Usefulness was assessed by the agent under the owner's explicit delegation; it is not an independent human study. One known candidate uncertainty-loss error remains visible in the measured accuracy. Candidate review remains part of the workflow.

The package is local and unpublished, with an ad-hoc linker signature. Public distribution signing/notarization and hosted CI execution require separate setup. Other platforms, universal live collection, a production daemon, browser extension, full TUI, autonomous enforcement, cloud sync and mobile remain future work. [Acceptance evidence](readiness.md) · [Platform policy](platforms.md).

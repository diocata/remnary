# Security and privacy

Remnary has a tested local v0.1 release for the documented Mac target and has no independent security audit. Do not post personal exports, databases, keys, bearer URLs, model payloads or source quotations in public issues.

The default CLI uses SQLCipher for conversations, metadata, derived records and FTS in one encrypted archive. macOS Keychain stores the key by archive location; explicit private recovery files are an alternative. Keys use OS randomness, are never printed, and are zeroized where supported by owned Rust buffers. New private files use restrictive Unix permissions. Native libraries, operating-system memory and copied buffers are not a guaranteed zeroization boundary.

Wrong/missing keys fail closed. Existing archives never silently receive replacement keys. Export a recovery key and keep it separate from backups. Encrypted backups are validated; restore requires a new target. Key changes use a new-key backup/new-archive workflow. Explicit plaintext conversion uses a separate keyed export while preserving the read-only original. No in-place key change or automatic Keychain reassignment is performed. [Recovery instructions and remaining crash-test limits](docs/recovery.md).

Only selected conversation sources are imported. Filename/size detection is separate. The application does not read account credentials, capture keystrokes, enable cloud sync, contact AI accounts or send telemetry. The optional adapter sends redacted normalized content only to a trusted loopback Ollama daemon and refuses advertised cloud models before sending history. Disable Ollama cloud features separately. An arbitrary daemon can lie about forwarding.

Redaction covers tested credential patterns and common contact forms, not every possible secret or identifying detail. Evidence checks prevent invented citations; they cannot prove that a pasted sentence expresses the owner's views. Findings require review before profile confirmation. Sensitive inferred traits are restricted.

## Plaintext and deletion boundaries

CLI output, browser content, process memory and explicitly generated Markdown contain readable personal information. Protect those surfaces and export locations. Original provider logs remain unchanged with their original privacy characteristics.

The dashboard requires its random bearer token. Its startup link uses a URL fragment; the UI removes it from the address bar and retains it in tab session storage. Do not share it. Stop the process to revoke the session. A process with access to the token, terminal or browser can read the unlocked archive.

Exclusion hides retained conversations; deletion removes canonical revisions, search entries and derived dependencies. Identity-only tombstones prevent silent reintroduction. Durable projection cleanup preserves user-authored sections; changed/unsafe files remain conflicts. Independent copies, original exports and previous backups require separate removal. SQLite secure deletion does not guarantee forensic erasure from SSDs, snapshots, swap or copies.

## Reporting

The source repository is [diocata/remnary](https://github.com/diocata/remnary). Public issues are suitable only for nonsensitive synthetic reproductions and affected commit IDs. Contact the repository owner privately for sensitive reports; a dedicated private reporting channel has not been configured by this project. Publishing source does not establish an independent security audit or a signed/notarized binary distribution.

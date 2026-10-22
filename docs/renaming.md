# Remnary naming and existing archives

Remnary is the project name, Rust crate and CLI executable. New archives use `remnary.sqlite3`, new macOS Keychain entries use `dev.remnary`, and the default data directory is `~/Library/Application Support/remnary`. New snapshots and managed Markdown use the Remnary namespace. The dashboard, release filenames and commands use the same name.

The previous development name was Observr. Compatibility identifiers retain that spelling only where needed to read existing data; they are not current branding. Kiro message-origin hash domains also remain unchanged so existing evidence identities stay stable. The database application ID and schema remain unchanged so encrypted backups stay compatible.

## Keep using an existing archive

Use `remnary --data-dir /path/to/previous/archive ...` to select it explicitly. Remnary recognizes an existing `observr.sqlite3` without renaming it or its live SQLite sidecars. Native Keychain mode uses that archive's original `dev.observr` service and location identity; macOS may require access approval for the new executable. An existing private `--key-file` remains valid. If both old and new database filenames exist in one directory, commands stop instead of choosing silently.

To move to the new filename and location, create an encrypted backup from the existing archive and restore it into an unused directory with its original recovery key. The restored file is named `remnary.sqlite3`. Follow [the backup and restore instructions](recovery.md); moving a directory alone does not move location-bound Keychain access. Original databases and backup files are preserved. Restored archives do not adopt old Markdown projection ownership; generate a new vault in a new directory.

Old partial-view metadata remains recognized as partial. Existing managed vault markers and explicit feedback blocks remain readable. Synchronization upgrades verified managed markers to the new name while preserving owner annotations; edited generated sections still conflict. New review blocks use `remnary-feedback`.

Earlier release hashes, model evaluations and performance measurements describe the commits where they were recorded. The rename does not retroactively change those binaries or establish new model-quality measurements. The renamed build has its own package metadata and tests, including old-name archive/backup, ambiguous filenames, partial metadata, managed-note upgrades and feedback compatibility.

Before the first source push, unpublished commits were rewritten to split synthetic vendor-token fixtures into compile-time fragments. Their runtime test values and redaction coverage are unchanged; no push-protection exception is used. A private local Git bundle preserves the original development history. Historical evaluation commit hashes refer to that original history and its locally retained artifacts; current release metadata identifies the published history.

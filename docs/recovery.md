# Upgrade, recovery and key changes

Stop commands/writers using the old archive while preparing a cutover. Keep the original and recovery key until the new archive has passed integrity, search, evidence and review checks. These workflows publish a new copy; they do not replace or delete the original. Choose a private output directory and keep keys separate from backups.

## Upgrade a plaintext development archive

Default encrypted commands refuse plaintext archives. Explicitly select the legacy source and create a new private key:

```sh
observr key create /private/keys/new.key
observr --insecure-plaintext --data-dir /path/to/legacy encrypt-copy /private/backups/encrypted.sqlite3 --new-key-file /private/keys/new.key
observr --data-dir /path/to/new-archive --key-file /private/keys/new.key restore /private/backups/encrypted.sqlite3
observr --data-dir /path/to/new-archive --key-file /private/keys/new.key doctor
observr --data-dir /path/to/new-archive --key-file /private/keys/new.key search "words you recognize"
```

`encrypt-copy` opens the source read-only, uses SQLCipher's logical export into a keyed private stage and upgrades only that encrypted copy. This preserves source versions 1–4 without migrating the original file in place. It carries evidence, FTS, feedback and tombstones, clears external projection ownership, verifies integrity and publishes without replacing an existing file. Export has a 30-second interrupt deadline. Wrong/foreign/incompatible sources fail; no incomplete destination is published. An abrupt process kill may leave an encrypted temporary stage; the original remains available.

New keys are required explicitly; conversion does not overwrite Keychain entries or infer a key for the old plaintext archive. Use `--key-file` for subsequent commands on the converted archive. Normal backup cannot mix plaintext/encrypted storage modes. This workflow follows [SQLCipher's supported plaintext export method](https://www.zetetic.net/sqlcipher/sqlcipher-api/#sqlcipher_export), with keys applied through the C API rather than interpolated into SQL.

## Recover an encrypted archive

```sh
observr key export /private/keys/recovery.key
observr backup /private/backups/archive.sqlite3
observr --data-dir /path/to/recovered --key-file /private/keys/recovery.key restore /private/backups/archive.sqlite3
observr --data-dir /path/to/recovered --key-file /private/keys/recovery.key doctor
```

Use the key that encrypted the selected backup. Restore requires a new destination and validates before publication. Supported older schema versions upgrade only on the private destination stage; the original backup is opened read-only. A wrong key or incompatible/corrupt backup leaves the original archive and existing destinations untouched. Restored ownership registries are cleared: generate a new vault explicitly instead of adopting or deleting the original installation's files.

## Change encryption keys by creating a new archive

```sh
observr key create /private/keys/rotated.key
observr backup /private/backups/rotated.sqlite3 --backup-key-file /private/keys/rotated.key
observr --data-dir /path/to/rotated-archive --key-file /private/keys/rotated.key restore /private/backups/rotated.sqlite3
observr --data-dir /path/to/rotated-archive --key-file /private/keys/rotated.key doctor
```

Add your existing `--data-dir`/`--key-file` to the backup command if the old archive uses nondefault settings. The new archive opens with the new key; the old key cannot open it. The old archive remains independently readable with its old key. This avoids an interruption window between rewriting the original and updating an OS key entry. There is no in-place `rekey` command or automatic Keychain reassignment; use the new directory/key-file explicitly after cutover.

Changing keys does not revoke old backups, copies, plaintext exports or their keys. Retire those separately after verifying recovery. Original plaintext archives and vaults stay plaintext until you choose how to remove or protect them. No forensic-erasure guarantee applies to SSDs, snapshots or independent copies.

## Verification evidence

Tests cover version-1 conversion with the original bytes/schema unchanged, current-schema conversion with feedback retained, wrong keys, existing destination refusal, Unicode/punctuation paths, FTS search, resumed analysis, independent vault ownership, encrypted backup under a different key and CLI recovery using only that new key. A manual release-binary check (`python3 scripts/check-recovery.py`) observed nonempty private stages and killed backup, restore and plaintext conversion during writes. No incomplete destination was published, originals retained their exact file hashes, encrypted private orphan stages remained, and retry/restore/integrity/search passed for all three. An orphan stage from a killed process is not a usable backup; retain the original and retry the command. This covers three observed SIGKILL windows, not hardware power loss, every publication boundary or vault interruption. Those broader checks remain open.

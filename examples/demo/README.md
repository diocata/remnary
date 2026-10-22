# A fictional history you can explore

This is the exact public dataset used in the README screenshots and animations. It contains **three fictional ChatGPT-format conversations and 28 messages**. No account exports, real user text or provider credentials are included. The JSON fixture is original Remnary material under the project's MIT OR Apache-2.0 license choice.

## Create an isolated archive

From the repository root, after installing Remnary:

```sh
python3 scripts/demo.py
```

For a local build, use `python3 scripts/demo.py --binary ./target/release/remnary` instead. The script creates an unused private `.remnary/demo/` directory containing:

```text
demo/
├── recovery.key       Private key file; never put this in Git
├── archive/           SQLCipher database and archive locks
└── vault/             Explicitly generated plaintext Markdown
```

It refuses to overwrite an existing directory. Use `--directory .remnary/demo-2` to start another independent demo. A failure preserves its partial directory for inspection; choose a new directory when retrying.

## Expected results

| Step | Actual expected result |
| --- | --- |
| Import | `3 new, 0 updated, 0 unchanged, 0 skipped conversations` |
| Rules analysis | Three conversations processed; twelve source extractions persisted. |
| Discovery | Nine grouped observations. |
| Repeated preference | “I prefer Rust for local tools.” has two citations in two conversations. |
| Recurring comparison pattern | Four distinct comparison requests in two conversations; inferred, not automatically confirmed. |
| Decision timeline | Considering SQLite and choosing SQLite remain distinct events. |
| Initial profile | Empty: analysis does not confirm a personal fact. |
| Search for `SQLite` | Five matching messages, each with an exact source revision. |
| Integrity | Database integrity and foreign keys pass; schema 5. |

## Walk through the same UI

The script prints a complete `web` command with the selected archive and key-file paths. Run it, then open the entire loopback link it prints. The link's session token is private; it is never part of the public screenshots.

1. **Discoveries:** select the Rust preference. Its two original owner messages remain attached.
2. **Open this source revision:** inspect the original user/assistant roles, message IDs and revision.
3. **Confirm:** explicitly confirm the fictional Rust preference.
4. **Profile:** only that reviewed finding appears. This confirmation changes the demo archive, not any real history.
5. **Search:** search for `SQLite` and open a matching source.
6. **Timeline:** compare “I am considering…” with “I chose…”.

The README GIF is a timed sequence of these captured UI states, not a claim about interaction latency. Static PNGs are available in [the media gallery](../../docs/assets/README.md).

## Try the CLI

For the default demo directory, define a shell helper that selects it on every command:

```sh
demo() {
  remnary --data-dir "$PWD/.remnary/demo/archive" \
    --key-file "$PWD/.remnary/demo/recovery.key" "$@"
}

demo status
demo discover --all --limit 100
demo patterns
demo patterns --topics
demo search SQLite
demo timeline
demo today --date 2026-10-18 --timezone UTC
```

The fictional Rust-preference ID is stable for this fixture:

```sh
demo feedback 3dd7f9ff4a5e2df06ff781d95887e2111642aa0a41d68f2b9d7cbe939866de4a confirm
demo profile
demo vault sync "$PWD/.remnary/demo/vault" --timezone UTC
```

Sync the vault after feedback changes. Review/deletion can invalidate and clean generated notes, so the filesystem projection is not automatically current until synchronized. Owner annotations outside generated sections survive a normal sync.

Repeat the import and analysis:

```sh
demo import examples/demo/conversations.json
demo analyze
```

The import reports **zero new, zero updated, three unchanged**. Cached analysis reports no pending work. No model is installed, downloaded or contacted in this workflow.

## Boundaries

The dataset illustrates a small English rules workflow. It is not a model benchmark, provider account export or personal profile of the project owner. The generated Markdown is plaintext. Demo keys, archives, runtime logs and captures stay under ignored `.remnary/`; only the fictional input, reviewed media and sanitized transcript belong in Git.

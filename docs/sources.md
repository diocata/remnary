# Supported local sources

Observr imports files you select. It does not sign into providers, read credentials, monitor applications or enable background collection. `observr sources --detect` inventories filenames and sizes in the three locations below; it reads no message bodies and needs no initialized archive. Import is a separate action.

| Source | Import input | Detection on macOS | Verification |
| --- | --- | --- | --- |
| ChatGPT | Mapping-format `conversations.json`, or ZIP containing exactly one such file | No export found in the selected local search | Synthetic JSON/ZIP fixtures; a real account export remains unverified |
| Claude Code | One JSONL file or selected session directory | `~/.claude/projects` | 17 local files imported; synthetic regression and encrypted integration tests |
| Codex | One rollout JSONL file or selected session directory | `~/.codex/sessions` | 11 local files imported; forked/inherited metadata regression tests |
| Kiro IDE | One `.chat` JSON file or selected chat directory | `~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent` | Eight local snapshots coalesced into four chats; synthetic regression tests |

These private application formats can change. Local compatibility evidence is from this machine on 2026-10-20, rather than a promise about every application version. Unknown formats fail with a diagnostic instead of being reported as connected. Claude consumer exports, Kiro CLI exports, Cursor databases, Gemini and other account histories are currently unsupported. Finding an installed application does not provide access to its account history.

```sh
observr sources --detect
observr import ~/Downloads/conversations.json
observr import ~/.claude/projects --source claude-code
observr import ~/.codex/sessions --source codex
observr import "$HOME/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent" --source kiro
observr analyze
observr discover --all
```

Select `--source` for directory imports. File imports may detect their format automatically. Directory traversal skips symlinks, accepts only the selected format's extension and is bounded to 10,000 files / 100,000 entries. Inputs are limited to 256 MiB per file, 512 MiB per local batch and 16 MiB per JSONL record. A malformed or incomplete record fails the batch before any database mutation; retry after the originating application finishes writing. Imports never modify source files.

## Attribution and retained evidence

The normalized archive retains message IDs, source metadata and original records. Missing dates stay unknown. Binary attachments are not fetched or analyzed. Unknown roles are retained without treating them as user speech. Tool payloads remain source metadata where a format lacks plain text.

Claude Code's `origin.kind` distinguishes human messages from generated task notifications. Sidechain/agent requests and tool results cannot supply personal facts. Codex uses the rollout's first session identity, retains child identity across inherited parent metadata and marks subagent requests and internal context separately. Event-message copies of response messages are not imported twice.

Kiro's observed format labels both its system prompt and genuine requests `human`. Observr recognizes the provider identity prompt as system content, separates the known environment suffix from normalized user text, and retains the full original entry in metadata. It coalesces only exact cumulative message prefixes under one execution ID. Divergent snapshots fail for review. Source snapshot metadata is retained, session time spans use the available generation times and individual message times remain unknown.

These guards cannot establish that every passage inside a human message is the person's own statement. Pasted transcripts, examples and quoted instructions still need semantic review. A valid quote establishes provenance; it does not establish endorsement. The deterministic rules are deliberately limited and all extracted findings remain candidates until reviewed.

## Real-data check

The authorized local evaluation imported 32 conversations from the sources above. It exposed and fixed provider-instruction and task-notification attribution errors, and repeated unchanged imports and analysis were exercised. After those corrections, the rules produced one remaining candidate from a pasted conversation; no useful recurring pattern or human-confirmed profile was established. This is evidence that the import path works and that richer extraction still needs evaluation, not evidence that the product's usefulness target has passed. Private sources, keys and results are ignored by Git.

Provider references: [Claude Code sessions](https://code.claude.com/docs/en/sessions), [Kiro IDE chat](https://kiro.dev/docs/ide/chat/), [Kiro CLI commands](https://kiro.dev/docs/cli/reference/slash-commands/). The Kiro IDE `.chat` adapter follows the locally observed format; its presence does not imply support for the separately documented CLI export formats.

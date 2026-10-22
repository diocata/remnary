# Supported local sources

Remnary imports files you select. It does not sign into providers, read credentials, monitor applications or enable background collection. `remnary sources --detect` inventories filenames and sizes in the three locations below; it reads no message bodies and needs no initialized archive. Import is a separate action.

| Source | Import input | Detection on macOS | Verification |
| --- | --- | --- | --- |
| ChatGPT | Mapping-format `conversations.json`, or ZIP containing one such file or numbered `conversations-*.json` shards | No owner export found in the selected local/indexed filename search | Synthetic JSON/ZIP/shard fixtures plus five published real records with exact reconstruction; [compatibility scope](import-compatibility.md) |
| Connected app history views | Selected Codex-app `read_thread` JSON snapshot | Available through this assistant session's connected history tools, rather than a filesystem scan | 40 selected ChatGPT views imported; partial coverage and truncation retained |
| Claude Code | One JSONL file or selected session directory | `~/.claude/projects` | 17 local files imported; synthetic regression and encrypted integration tests |
| Codex | One rollout JSONL file or selected session directory | `~/.codex/sessions` | 11 local files imported; forked/inherited metadata regression tests |
| Kiro IDE | One `.chat` JSON file or selected chat directory | `~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent` | Eight local snapshots coalesced into four chats; synthetic regression tests |

These private application formats can change. Local compatibility evidence is from this machine on 2026-10-20, rather than a promise about every application version. Unknown formats fail with a diagnostic instead of being reported as connected. Claude consumer exports, Kiro CLI exports, Cursor databases, Gemini and other account histories are currently unsupported. Finding an installed application does not provide access to its account history.

## Partial connected-app views

`remnary import-app-snapshot /path/to/snapshot.json` accepts a selected ChatGPT `read_thread` result from Codex app. Codex task views are refused here because their human/subagent attribution is not established by this view; use local Codex JSONL instead. `remnary import-app-snapshot -` reads one compact JSON line from standard input, allowing a caller to send an authorized tool result directly into encryption without a plaintext export file. The Rust application does not itself connect to Codex/ChatGPT accounts; these tools are available to the assistant in this app session.

Provider/message IDs and source text are retained. Message timestamps/parents absent from the view stay unknown; turn timing remains metadata. Partial history, pagination and truncated items remain explicit, including in the dashboard/transcripts. Truncated user items cannot supply personal findings. Snapshots never replace a full provider export, and an updated partial view must retain every previously imported message ID. Combine source pages before importing a larger view. Identical snapshots are unchanged. This does not validate ChatGPT's account-export ZIP format against real data or imply access to every account conversation.

```sh
remnary sources --detect
remnary import ~/Downloads/conversations.json
remnary import ~/.claude/projects --source claude-code
remnary import ~/.codex/sessions --source codex
remnary import "$HOME/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent" --source kiro
remnary analyze
remnary discover --all
```

Select `--source` for directory imports. File imports may detect their format automatically. Directory traversal skips symlinks, accepts only the selected format's extension and is bounded to 10,000 files / 100,000 entries. Inputs are limited to 256 MiB per file, 512 MiB per local batch and 16 MiB per JSONL record. A malformed or incomplete record fails the batch before any database mutation; retry after the originating application finishes writing. Imports never modify source files.

## Native Claude evaluation captures

A separate authorized evaluation uses fourteen Claude consumer conversations captured through the signed-in native application's accessibility view. This is an assistant-operated private evaluation path, not an automatic collector or a supported consumer-export CLI importer. Account-export compatibility remains unverified.

The captures retain conversation UUIDs and explicit “You said” / “Claude responded” attribution. UI ordinal message identities are labelled as such; provider message UUIDs, timestamps, parents and binary attachments are unavailable. Unattributed tool-only messages cannot supply personal facts. Visible text and the raw accessibility message records are preserved privately. Owner text must agree across overlapping captures; differences fail normalization. Assistant rendering variants are retained in the raw captures.

The initial captures omitted virtualized positions in four chats. Further scrolling recovered all reported positions: 140 messages, including 70 owner messages. All native captures remain partial: attachments and unexposed context remain unavailable. The normalized `claude-native-view-v1` collection marker receives the same protection as connected views: partial imports cannot replace complete exports or drop previously retained messages, and the vault/dashboard disclose partial coverage. A regression verifies these safeguards. The separate fourteen-source model evaluation completed and received agent source review; its six retained findings provide no recurring group. The expanded integrated check completes 86 sources / 3,364 messages with 31 retained findings and no pending sources or repeated-claim group. One broad related-topic thread is accurate but not rated a useful recurring discovery. All fourteen native sources show partial coverage and unknown dates in the dashboard. See [current evidence](readiness.md).

## Attribution and retained evidence

The normalized archive retains message IDs, source metadata and original records. Missing dates stay unknown. Binary attachments are not fetched or analyzed. Unknown roles are retained without treating them as user speech. Tool payloads remain source metadata where a format lacks plain text.

Claude Code's `origin.kind` distinguishes human messages from generated task notifications. Sidechain/agent requests and tool results cannot supply personal facts. Codex uses the rollout's first session identity, retains child identity across inherited parent metadata and marks subagent requests and internal context separately. Event-message copies of response messages are not imported twice.

Kiro's observed format labels both its system prompt and genuine requests `human`. Remnary recognizes the provider identity prompt as system content, separates the known environment suffix from normalized user text, and retains the full original entry in metadata. It coalesces only exact cumulative message prefixes under one execution ID. Divergent snapshots fail for review. Source snapshot metadata is retained, session time spans use the available generation times and individual message times remain unknown.

These guards cannot establish that every passage inside a human message is the person's own statement. Pasted transcripts, examples and quoted instructions still need semantic review. A valid quote establishes provenance; it does not establish endorsement. The deterministic rules are deliberately limited and all extracted findings remain candidates until reviewed.

## Real-data check

The completed authorized v36 evaluation corpus contains 72 conversations, 3,224 current messages and 79 retained revisions from local Claude Code, Codex and Kiro files and selected partial ChatGPT views. Earlier imports exposed provider-instruction, task-notification and pasted-transcript attribution errors; the resulting safeguards, reimports and incremental analysis were exercised. Current model-quality results and open acceptance gates are recorded in [readiness.md](readiness.md). Useful real recurring patterns remain unestablished. The owner delegated usefulness assessment to the agent; those ratings are not independent human research or owner confirmation. Private sources, keys and results are ignored by Git.

Provider references: [Claude Code sessions](https://code.claude.com/docs/en/sessions), [Kiro IDE chat](https://kiro.dev/docs/ide/chat/), [Kiro CLI commands](https://kiro.dev/docs/cli/reference/slash-commands/). The Kiro IDE `.chat` adapter follows the locally observed format; its presence does not imply support for the separately documented CLI export formats.

ZIP conversation members are read directly without extraction. Numbered shards are processed in numeric order from one archive folder; mixed legacy/sharded inputs, duplicate shard numbers, conflicting conversation copies and multiple candidate folders are rejected before persistence. The compressed file and the combined actual uncompressed conversation bytes are each capped at 256 MiB, with at most 1,024 shards / 100,000 ZIP members. Shard names follow the numeric naming used by an [independent importer](https://github.com/luosx0403/chatgpt-sqlite-webui/blob/d558b965bfbf288dd1e32323edfdf249c8b6a6d8/chatgpt_export_archiver/scanner.py); synthetic tests verify the contract. Five published real records also pass generated ZIP/shard wrappers; [provenance and observed limits](import-compatibility.md). An original downloaded account ZIP remains unverified.


Kiro's execution ID is retained as a source-record identity. Different execution records can include the exact earlier history. Evidence counts correlate complete cumulative user/assistant dialogue prefixes with substantive copied assistant replies; tool frames, empty streaming entries and native timestamp metadata do not establish independent dialogue. They retain raw records/links rather than treating copied questions as independently repeated user behavior. Identical prompts and short greetings alone remain separate. Non-prefix forks and cross-provider copies are not exhaustively resolved. [Counting policy](architecture.md).


The observed Kiro steering-initialization template is excluded from personal evidence by exact LF/CRLF prefix fingerprints. Owner text appended after a recognized prefix remains eligible, and the raw message is retained. This is observed-template compatibility, not universal recognition of future provider prompts. Source-origin correlation requires a substantive reply after eligible owner text; machine initialization frames alone cannot make independent owner requests look copied.

The observed Kiro Included Rules workspace envelope also supplies machine context under a native human role. A normalized guidance fingerprint and matching header/tag identify consecutive blocks; original owner text following them stays eligible. Raw source records remain unchanged. This is bounded compatibility with the observed envelope, not universal recognition of future provider formats.

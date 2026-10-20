# Importing corrections from Markdown

Observr preserves your text outside generated sections. It does not silently interpret free-form edits as profile facts. To make a correction authoritative, add one explicit review block outside the generated markers, or in a separate Markdown note you author.

Find the observation's `id` and current `feedback_count` using `observr discover --all --json`. Use their exact values:

````markdown
My correction:

```observr-feedback
{
  "observation_id": "REPLACE_WITH_THE_64_CHARACTER_OBSERVATION_ID",
  "action": "correct",
  "expected_feedback_count": 0,
  "text": "Prefer simple implementations for small personal projects."
}
```
````

The example count is a placeholder: use the current count from your archive. Preview and explicitly apply your chosen file:

```sh
observr vault apply-feedback /path/to/Profile.md --check
observr vault apply-feedback /path/to/Profile.md
observr vault sync /path/to/vault
```

`--check` validates current evidence and the expected review count, then prints the proposed action without changing the archive. Apply records ordinary authoritative feedback and triggers the same projection cleanup as CLI/dashboard review. It preserves the input's user-authored bytes. A second application fails as stale, rather than creating duplicate feedback. A concurrent reviewer cannot be overwritten: the count is rechecked inside the write transaction.

Actions are `confirm`, `reject`, `correct`, `dismiss`, `supersede` and `reset`. Only `correct` accepts/requires nonempty `text` (maximum 16 KiB). For other actions omit `text` or set it to null. Never simply increase a stale count without reviewing current knowledge first.

Exactly one top-level `observr-feedback` fence per selected file is accepted. Generated sections, quoted transcript fences and nested code examples do not execute as review. Multiple actions, invalid/unknown JSON fields, incomplete fences, missing evidence, stale counts, symlinks and files larger than 1 MiB fail safely. No background synchronization applies feedback automatically. Editing an owned generated body continues to report a preservation conflict; move your correction outside it or use a separate review note.

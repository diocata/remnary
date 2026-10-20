# Persistent exclusions

Exclusion retains existing canonical data encrypted but hides matching conversations from search, analysis, reports, source inspection and generated knowledge. To purge retained data instead, use `delete conversation ID`. Original provider files, independent exports and backups are outside these controls.

```sh
observr privacy add provider claude-code
observr privacy add project /absolute/path/to/project
observr privacy add directory /absolute/path/to/session-files
observr privacy add topic "private subject"
observr privacy list
observr privacy remove RULE_ID
```

Add your archive's `--data-dir` and `--key-file` options when applicable. The add command reports how many retained conversations became newly excluded, and reports pending projection conflicts. List shows rule IDs, scopes and values; remove accepts the ID. Exclusions are persisted inside the encrypted database and survive backup/restore.

Matching is deliberately explicit:

- Provider matches the normalized provider name exactly (ASCII case-insensitive).
- Project matches a recorded `cwd` path and its descendants. It does not guess project names from conversation text. Sources without recorded `cwd` cannot match a project rule.
- Directory matches recorded `cwd`, `source_path` or any cumulative `source_paths` entry, including descendants. `/work/app` does not match `/work/app-other`.
- Topic is an ASCII case-insensitive **literal substring** in the retained title or any message, including assistant messages. It excludes the whole conversation. It does not perform semantic topic classification, Unicode case folding or regular-expression/glob matching. `%`, `_` and SQL punctuation are literal.

Path rules require absolute paths, reject `..`, and normalize redundant separators and `.`. Paths compare as recorded, case-sensitively; symlink aliases and differently spelled historical paths are not automatically resolved. New local-session imports record absolute source-file paths. Old records with missing or relative metadata need an explicit conversation/provider rule or a fresh import with absolute paths. Inspect the affected count and your remaining sources when setting up a path rule.

A rule immediately hides current retained matches and queues generated content for durable cleanup. User-authored annotations survive; edited generated sections remain pending conflicts for explicit review. Removing a rule does not regenerate plaintext notes. Run `vault sync` explicitly to publish them again. Manual conversation exclusions and overlapping rules remain independent: `include ID` only removes that conversation's manual exclusion.

Future matching normalized conversations are skipped before canonical persistence or model analysis. The import parser still reads the explicitly selected input into bounded process memory to identify conversations and apply these rules; exclusions are not a filesystem access sandbox. Matching incoming content does not replace an older retained revision. Sources already excluded by a rule remain excluded during reimport until that rule is removed.

There is no background collector yet, so future collection-session pause controls do not apply. Stop the foreground import/model command to interrupt it; completed per-conversation analysis commits remain available.

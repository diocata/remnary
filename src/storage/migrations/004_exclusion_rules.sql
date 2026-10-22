-- Keep manual exclusions separate so removing one rule cannot undo another.
CREATE TABLE manual_excluded_sources (
    conversation_id TEXT PRIMARY KEY NOT NULL,
    excluded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO manual_excluded_sources SELECT * FROM excluded_sources;
DROP TABLE excluded_sources;
CREATE TABLE exclusion_rules (
    id TEXT PRIMARY KEY NOT NULL,
    scope TEXT NOT NULL CHECK(scope IN ('provider', 'project', 'directory', 'topic')),
    value TEXT NOT NULL CHECK(length(value) BETWEEN 1 AND 4096),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(scope, value)
);

-- Literal matching: no SQL LIKE wildcards or user regular expressions.
-- Project is a recorded cwd subtree; directory also covers source files.
CREATE VIEW excluded_sources AS
SELECT conversation_id FROM manual_excluded_sources
UNION
SELECT c.id FROM conversations c
JOIN conversation_revisions r ON r.conversation_id = c.id AND r.revision = c.current_revision
WHERE EXISTS (
    SELECT 1 FROM exclusion_rules x WHERE
    (x.scope = 'provider' AND lower(c.provider) = x.value)
    OR (x.scope = 'topic' AND (
        instr(lower(coalesce(r.title, '')), x.value) > 0
        OR EXISTS (SELECT 1 FROM messages m WHERE m.conversation_id = c.id
            AND m.revision = c.current_revision AND instr(lower(m.content), x.value) > 0)
    ))
    OR (x.scope IN ('project', 'directory') AND (
        json_extract(r.metadata, '$.cwd') = x.value
        OR substr(json_extract(r.metadata, '$.cwd'), 1, length(x.value) + 1) = x.value || '/'
        OR x.value = '/' AND substr(json_extract(r.metadata, '$.cwd'), 1, 1) = '/'
    ))
    OR (x.scope = 'directory' AND (
        json_extract(r.metadata, '$.source_path') = x.value
        OR substr(json_extract(r.metadata, '$.source_path'), 1, length(x.value) + 1) = x.value || '/'
        OR x.value = '/' AND substr(json_extract(r.metadata, '$.source_path'), 1, 1) = '/'
        OR EXISTS (SELECT 1 FROM json_each(json_extract(r.metadata, '$.source_paths')) p
            WHERE p.type = 'text' AND (p.value = x.value
                OR substr(p.value, 1, length(x.value) + 1) = x.value || '/'
                OR x.value = '/' AND substr(p.value, 1, 1) = '/'))
    ))
);

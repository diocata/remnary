-- Persist the selected completed run instead of relying on timestamps or
-- SQLite rowids at read time (VACUUM may change implicit rowids).
CREATE TABLE active_analysis_runs (
    conversation_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    engine TEXT NOT NULL,
    model TEXT NOT NULL,
    run_id TEXT NOT NULL UNIQUE REFERENCES analysis_runs(id) ON DELETE CASCADE,
    PRIMARY KEY (conversation_id, revision, engine, model),
    FOREIGN KEY (conversation_id, revision)
        REFERENCES conversation_revisions(conversation_id, revision) ON DELETE CASCADE
);

-- Older releases had no explicit head. Preserve the most recently inserted
-- retained result for each source/engine/model as the migration baseline.
INSERT INTO active_analysis_runs (conversation_id, revision, engine, model, run_id)
SELECT r.conversation_id, r.revision, r.engine, r.model, r.id FROM analysis_runs r
WHERE NOT EXISTS (
    SELECT 1 FROM analysis_runs newer
    WHERE newer.conversation_id = r.conversation_id AND newer.revision = r.revision
      AND newer.engine = r.engine AND newer.model = r.model AND newer.rowid > r.rowid
);

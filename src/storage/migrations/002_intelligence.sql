CREATE TABLE analysis_runs (
    id TEXT PRIMARY KEY NOT NULL,
    conversation_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    engine TEXT NOT NULL,
    processor_version TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    completed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (conversation_id, revision) REFERENCES conversation_revisions(conversation_id, revision) ON DELETE CASCADE,
    UNIQUE (conversation_id, revision, engine, processor_version, model, prompt_version)
);

CREATE TABLE observations (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    inference TEXT NOT NULL CHECK (inference IN ('explicit', 'inferred')),
    subject TEXT NOT NULL,
    topic TEXT NOT NULL,
    canonical_value TEXT NOT NULL,
    context TEXT,
    summary TEXT NOT NULL,
    decision_event TEXT,
    status TEXT NOT NULL DEFAULT 'candidate' CHECK (status IN (
        'candidate', 'supported', 'user_confirmed', 'user_rejected', 'superseded', 'dismissed'
    )),
    user_value TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE evidence (
    observation_id TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    message_id TEXT NOT NULL,
    quote TEXT NOT NULL,
    PRIMARY KEY (observation_id, conversation_id, revision, message_id, quote),
    FOREIGN KEY (conversation_id, revision, message_id)
        REFERENCES messages(conversation_id, revision, source_id) ON DELETE CASCADE
);
CREATE INDEX evidence_source ON evidence(conversation_id, revision, message_id);

CREATE TABLE observation_extractions (
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(id) ON DELETE CASCADE,
    observation_id TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    payload TEXT NOT NULL,
    PRIMARY KEY (analysis_run_id, observation_id)
);

CREATE TABLE user_feedback (
    id INTEGER PRIMARY KEY,
    observation_id TEXT NOT NULL REFERENCES observations(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK (action IN ('confirm', 'reject', 'correct', 'dismiss', 'supersede', 'reset')),
    value TEXT,
    previous_status TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX feedback_observation ON user_feedback(observation_id, id);

-- Retaining only the stable identity prevents imports resurrecting purged sources.
CREATE TABLE deleted_sources (
    conversation_id TEXT PRIMARY KEY NOT NULL,
    deleted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE excluded_sources (
    conversation_id TEXT PRIMARY KEY NOT NULL,
    excluded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE deleted_observations (
    observation_id TEXT PRIMARY KEY NOT NULL,
    deleted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE managed_files (
    path TEXT PRIMARY KEY NOT NULL,
    content_hash TEXT NOT NULL,
    generated_hash TEXT
);
CREATE TABLE managed_file_sources (
    path TEXT NOT NULL REFERENCES managed_files(path) ON DELETE CASCADE,
    conversation_id TEXT NOT NULL,
    PRIMARY KEY (path, conversation_id)
);
CREATE TABLE managed_file_observations (
    path TEXT NOT NULL REFERENCES managed_files(path) ON DELETE CASCADE,
    observation_id TEXT NOT NULL,
    PRIMARY KEY (path, observation_id)
);
CREATE TABLE file_cleanup (
    path TEXT PRIMARY KEY NOT NULL,
    content_hash TEXT NOT NULL,
    generated_hash TEXT,
    reason TEXT NOT NULL
);

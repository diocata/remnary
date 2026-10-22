-- Version 1. Immutable source snapshots keep future evidence tied to revisions.
CREATE TABLE conversations (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL,
    source_id TEXT NOT NULL,
    current_revision INTEGER NOT NULL CHECK (current_revision > 0),
    UNIQUE (provider, source_id)
);

CREATE TABLE conversation_revisions (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    fingerprint TEXT NOT NULL,
    title TEXT,
    created_at REAL,
    updated_at REAL,
    metadata TEXT NOT NULL,
    imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (conversation_id, revision)
);

CREATE TABLE messages (
    id INTEGER PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    source_id TEXT NOT NULL,
    parent_source_id TEXT,
    position INTEGER NOT NULL CHECK (position >= 0),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at REAL,
    metadata TEXT NOT NULL,
    UNIQUE (conversation_id, revision, source_id),
    UNIQUE (conversation_id, revision, position),
    FOREIGN KEY (conversation_id, revision)
        REFERENCES conversation_revisions(conversation_id, revision) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE messages_fts USING fts5(
    content, content='messages', content_rowid='id', tokenize='unicode61'
);
CREATE TRIGGER messages_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER messages_delete AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;
CREATE TRIGGER messages_update AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, content) VALUES ('delete', old.id, old.content);
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;

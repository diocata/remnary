-- Suppress re-extraction from a deleted finding's cited source spans without
-- retaining the deleted claim, quote, summary or feedback text.
CREATE TABLE deleted_evidence_spans (
    conversation_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    message_digest TEXT NOT NULL CHECK(length(message_digest) = 64),
    start_offset INTEGER NOT NULL CHECK(start_offset >= 0),
    end_offset INTEGER NOT NULL CHECK(end_offset > start_offset),
    deleted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (conversation_id, message_id, message_digest, start_offset, end_offset)
);

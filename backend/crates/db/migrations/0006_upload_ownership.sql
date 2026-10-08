CREATE TABLE uploads (
    file_id uuid PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    protocol text NOT NULL CHECK (protocol IN ('native', 's3')),
    key text,
    multipart boolean NOT NULL,
    if_none_match boolean NOT NULL DEFAULT false,
    state text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'completing', 'cleaning')),
    expected_size bigint CHECK (expected_size >= 0),
    expected_etag text,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    CONSTRAINT uploads_protocol_shape CHECK (
        (protocol = 'native' AND key IS NULL AND multipart AND NOT if_none_match
            AND state IN ('completing', 'cleaning')
            AND expected_size IS NULL AND expected_etag IS NOT NULL)
        OR
        (protocol = 's3' AND key IS NOT NULL
            AND ((state = 'completing' AND expected_size IS NOT NULL AND expected_etag IS NOT NULL)
                OR (state IN ('open', 'cleaning') AND expected_size IS NULL AND expected_etag IS NULL)))
    )
);
CREATE INDEX uploads_recovery ON uploads(protocol, state, updated_at);

INSERT INTO uploads(file_id, protocol, key, multipart, if_none_match, state,
                    expected_size, expected_etag, created_at, updated_at)
SELECT file_id, 's3', key, multipart, if_none_match,
       CASE WHEN state = 'aborting' THEN 'cleaning' ELSE state END,
       expected_size, expected_etag, created_at, updated_at
FROM s3_uploads;

-- Conflicting legacy owners fail the transaction instead of discarding recovery material.
INSERT INTO uploads(file_id, protocol, multipart, state, expected_etag, created_at, updated_at)
SELECT file_id, 'native', true, state, expected_etag, created_at, updated_at
FROM native_multipart_completions;

DROP TABLE s3_uploads;
DROP TABLE native_multipart_completions;

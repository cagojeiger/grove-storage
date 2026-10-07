CREATE TABLE files (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id text NOT NULL REFERENCES clients(id),
    state text NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'active', 'deleted', 'reclaimed')),
    declared_size bigint NOT NULL CHECK (declared_size >= 0),
    content_type text,
    declared_md5 text,
    etag text,
    part_size bigint CHECK (part_size > 0),
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    committed_at timestamptz,
    deleted_at timestamptz,
    CHECK (state <> 'active' OR committed_at IS NOT NULL),
    CHECK (state <> 'deleted' OR (committed_at IS NOT NULL AND deleted_at IS NOT NULL))
);
CREATE INDEX files_client_idx ON files(client_id);
CREATE INDEX files_nonactive_idx ON files(state, created_at) WHERE state <> 'active';

CREATE TABLE locations (
    file_id uuid PRIMARY KEY REFERENCES files(id),
    storage_id text NOT NULL REFERENCES storages(id),
    object_key text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    UNIQUE (storage_id, object_key)
);

CREATE TABLE leases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    file_id uuid NOT NULL REFERENCES files(id),
    kind text NOT NULL CHECK (kind IN ('write', 'read')),
    state text NOT NULL DEFAULT 'issued'
        CHECK (state IN ('issued', 'committed', 'expired', 'canceled')),
    expires_at timestamptz NOT NULL,
    secret_hash text CHECK (secret_hash ~ '^sha256:[0-9a-f]{64}$'),
    uploaded_size bigint CHECK (uploaded_size >= 0),
    uploaded_md5 text,
    upload_id text,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now()
);
CREATE INDEX leases_expiry_idx ON leases(expires_at) WHERE state = 'issued';
CREATE INDEX leases_file_idx ON leases(file_id);

CREATE TABLE lease_parts (
    lease_id uuid NOT NULL REFERENCES leases(id) ON DELETE CASCADE,
    part_no integer NOT NULL CHECK (part_no >= 1),
    state text NOT NULL DEFAULT 'claimed' CHECK (state IN ('claimed', 'done')),
    uploaded_size bigint CHECK (uploaded_size >= 0),
    uploaded_md5 text,
    CHECK (state <> 'done' OR (uploaded_size IS NOT NULL AND uploaded_md5 IS NOT NULL)),
    PRIMARY KEY (lease_id, part_no)
);

CREATE TABLE s3_object_keys (
    client_id text NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    key text NOT NULL,
    file_id uuid NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    PRIMARY KEY (client_id, key)
);
CREATE INDEX s3_object_keys_file_idx ON s3_object_keys(file_id);

-- Completion ownership survives an interrupted provider operation.
CREATE TABLE s3_uploads (
    file_id uuid PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    key text NOT NULL,
    multipart boolean NOT NULL,
    if_none_match boolean NOT NULL DEFAULT false,
    state text NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'completing', 'aborting')),
    expected_size bigint CHECK (expected_size >= 0),
    expected_etag text,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    CONSTRAINT s3_upload_completion_fields CHECK (
        (state = 'completing' AND expected_size IS NOT NULL AND expected_etag IS NOT NULL)
        OR (state <> 'completing' AND expected_size IS NULL AND expected_etag IS NULL)
    )
);
CREATE INDEX s3_uploads_state_idx ON s3_uploads(state, updated_at);

CREATE TABLE native_multipart_completions (
    file_id uuid PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    state text NOT NULL DEFAULT 'completing' CHECK (state IN ('completing', 'cleaning')),
    expected_etag text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.transaction_now()
);
CREATE INDEX native_multipart_completions_state_idx
    ON native_multipart_completions(state, updated_at);

-- History is independent of resource deletion; IDs are snapshots, not FKs.
CREATE TABLE lease_history (
    at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    file_id uuid NOT NULL,
    storage_id text NOT NULL,
    client_id text NOT NULL,
    kind text NOT NULL CHECK (kind IN ('write', 'read')),
    size bigint NOT NULL CHECK (size >= 0)
);
CREATE INDEX lease_history_at_idx ON lease_history(at);
CREATE INDEX lease_history_file_idx ON lease_history(file_id, at);

CREATE TABLE usage_snapshots (
    day date NOT NULL,
    storage_id text NOT NULL,
    client_id text NOT NULL,
    active_bytes bigint NOT NULL CHECK (active_bytes >= 0),
    active_files bigint NOT NULL CHECK (active_files >= 0),
    PRIMARY KEY (day, storage_id, client_id)
);

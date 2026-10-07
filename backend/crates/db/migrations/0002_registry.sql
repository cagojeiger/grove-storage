CREATE TABLE storages (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]([a-z0-9-]{0,62}[a-z0-9])?$'),
    kind text NOT NULL DEFAULT 's3' CHECK (kind = 's3'),
    force_relay boolean NOT NULL DEFAULT false,
    endpoint text,
    public_endpoint text,
    region text,
    bucket text,
    force_path_style boolean NOT NULL DEFAULT false,
    access_key text,
    secret_key_ciphertext bytea,
    secret_key_nonce bytea CHECK (octet_length(secret_key_nonce) = 12),
    enc_key_id text,
    capacity_bytes bigint NOT NULL CHECK (capacity_bytes >= 0),
    metadata jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    updated_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    CONSTRAINT storages_s3_fields CHECK (
        endpoint IS NOT NULL AND public_endpoint IS NOT NULL
        AND region IS NOT NULL AND bucket IS NOT NULL
        AND access_key IS NOT NULL AND secret_key_ciphertext IS NOT NULL
        AND secret_key_nonce IS NOT NULL AND enc_key_id IS NOT NULL
    ),
    CONSTRAINT storages_metadata_check CHECK (
        jsonb_typeof(metadata) = 'object'
        AND NOT jsonb_path_exists(metadata, 'strict $.* ? (@.type() != "string")')
        AND octet_length(metadata::text) <= 8192
    )
);

CREATE TABLE clients (
    id text PRIMARY KEY CHECK (id ~ '^[a-z0-9]([a-z0-9-]{0,62}[a-z0-9])?$'),
    storage_id text NOT NULL REFERENCES storages(id),
    metadata jsonb NOT NULL DEFAULT '{}',
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now(),
    CONSTRAINT clients_metadata_check CHECK (
        jsonb_typeof(metadata) = 'object'
        AND NOT jsonb_path_exists(metadata, 'strict $.* ? (@.type() != "string")')
        AND octet_length(metadata::text) <= 8192
    )
);
CREATE INDEX clients_storage_idx ON clients(storage_id);

CREATE TABLE client_native_keys (
    key_hash text PRIMARY KEY CHECK (key_hash ~ '^sha256:[0-9a-f]{64}$'),
    client_id text NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now()
);
CREATE INDEX client_native_keys_client_idx ON client_native_keys(client_id);

CREATE TABLE client_s3_credentials (
    access_key_id text PRIMARY KEY CHECK (access_key_id ~ '^[a-z0-9]{8,64}$'),
    client_id text NOT NULL REFERENCES clients(id) ON DELETE CASCADE,
    secret_key_ciphertext bytea NOT NULL,
    secret_key_nonce bytea NOT NULL CHECK (octet_length(secret_key_nonce) = 12),
    enc_key_id text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT grove_time.transaction_now()
);
CREATE INDEX client_s3_credentials_client_idx ON client_s3_credentials(client_id);

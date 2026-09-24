CREATE TABLE management.master_configuration (
    id smallint PRIMARY KEY CHECK (id = 1),
    generation bigint NOT NULL CHECK (generation > 0),
    token_hash text NOT NULL CHECK (token_hash ~ '^[0-9a-f]{64}$')
);
CREATE INDEX sessions_master ON management.sessions(master_generation, created_at, id)
    WHERE auth_method = 'master';

ALTER TABLE storages ADD COLUMN metadata jsonb NOT NULL DEFAULT '{}';
ALTER TABLE clients ADD COLUMN metadata jsonb NOT NULL DEFAULT '{}';

ALTER TABLE storages ADD CONSTRAINT storages_metadata_check CHECK (
    jsonb_typeof(metadata) = 'object'
    AND NOT jsonb_path_exists(metadata, 'strict $.* ? (@.type() != "string")')
    AND octet_length(metadata::text) <= 8192
);
ALTER TABLE clients ADD CONSTRAINT clients_metadata_check CHECK (
    jsonb_typeof(metadata) = 'object'
    AND NOT jsonb_path_exists(metadata, 'strict $.* ? (@.type() != "string")')
    AND octet_length(metadata::text) <= 8192
);

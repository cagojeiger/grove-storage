-- Stop old writers before applying. Existing filesystem data must be migrated
-- explicitly; an unexpected FS row aborts this transaction without deleting it.
LOCK TABLE storages IN ACCESS EXCLUSIVE MODE;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM storages WHERE kind <> 's3') THEN
        RAISE EXCEPTION 'S3-only migration requires all filesystem storages to be migrated first';
    END IF;
END $$;

ALTER TABLE storages
    DROP CONSTRAINT storages_kind_check,
    DROP CONSTRAINT storages_s3_fields,
    DROP CONSTRAINT storages_fs_fields,
    DROP COLUMN root_path,
    ADD CONSTRAINT storages_kind_check CHECK (kind = 's3'),
    ADD CONSTRAINT storages_s3_fields CHECK (
        endpoint IS NOT NULL AND public_endpoint IS NOT NULL
        AND region IS NOT NULL AND bucket IS NOT NULL
        AND access_key IS NOT NULL AND secret_key_ciphertext IS NOT NULL
        AND secret_key_nonce IS NOT NULL AND enc_key_id IS NOT NULL
    );

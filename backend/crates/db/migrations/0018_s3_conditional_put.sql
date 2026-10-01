ALTER TABLE s3_uploads
    ADD COLUMN if_none_match boolean NOT NULL DEFAULT false;

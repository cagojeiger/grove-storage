-- Unknown context on existing history stays NULL rather than being inferred.
ALTER TABLE management.command_invocations
    ADD COLUMN target_type text,
    ADD COLUMN target_id text,
    ADD COLUMN stage text,
    ADD COLUMN auth_method text,
    ADD CONSTRAINT invocation_target CHECK (
        (target_type IS NULL AND target_id IS NULL) OR
        (target_type IS NOT NULL AND target_type IN ('account', 'credential', 'session', 'storage', 'client')
            AND target_id IS NOT NULL AND length(target_id) BETWEEN 1 AND 64)
    ),
    ADD CONSTRAINT invocation_stage CHECK (stage IN (
        'authentication', 'authorization', 'reauthentication', 'validation',
        'execution', 'commit', 'browser_admission'
    )),
    ADD CONSTRAINT invocation_auth_method CHECK (auth_method IN ('password', 'session', 'api_token'));

ALTER TABLE management.security_events
    ADD COLUMN operation text CHECK (length(operation) BETWEEN 1 AND 80),
    ADD COLUMN stage text CHECK (stage IN (
        'authentication', 'authorization', 'reauthentication', 'validation',
        'execution', 'commit', 'browser_admission'
    )),
    ADD COLUMN auth_method text CHECK (auth_method IN ('password', 'session', 'api_token')),
    ADD COLUMN is_sampled boolean;

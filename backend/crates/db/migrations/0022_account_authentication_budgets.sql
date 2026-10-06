CREATE TABLE management.authentication_budgets (
    account_id uuid NOT NULL REFERENCES management.accounts(id) ON DELETE CASCADE,
    purpose text NOT NULL CHECK (purpose IN ('login', 'reauthentication')),
    window_start timestamptz NOT NULL DEFAULT clock_timestamp(),
    attempts integer NOT NULL CHECK (attempts BETWEEN 1 AND 60),
    PRIMARY KEY (account_id, purpose)
);

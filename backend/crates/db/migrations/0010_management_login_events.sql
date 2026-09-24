ALTER TABLE management.security_events
    DROP CONSTRAINT security_events_event_type_check,
    DROP CONSTRAINT security_events_reason_code_check,
    ADD CONSTRAINT security_events_event_type_check CHECK (event_type IN (
        'authentication_failed', 'permission_denied', 'authentication_unavailable',
        'authentication_succeeded', 'authentication_rate_limited'
    )),
    ADD CONSTRAINT security_events_reason_code_check CHECK (reason_code IN (
        'unauthenticated', 'forbidden', 'unavailable', 'authenticated', 'rate_limited'
    ));

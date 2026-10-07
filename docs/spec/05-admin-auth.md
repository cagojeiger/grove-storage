# Retired Operator Authentication

The operator REST API, environment tokens, dedicated sessions and local `admin`
commands are removed. `/api/admin/v1` and its descendants return 410
`legacy_admin_removed` without authentication or database access.

The current management contract is [local Account authentication](11-local-management-auth.md):
password sessions for the console, named Account API tokens for CLI/MCP/resource
commands, and server-local `account init` / `account recover` for operator recovery.

Client Native/S3 credentials remain independent. Their file API contract is unchanged.
The [fresh installation baseline](../development/fresh-installation.md) contains
no old operator tables and does not upgrade an existing FileGate database.

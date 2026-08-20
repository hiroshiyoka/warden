CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id UUID REFERENCES tenants(id),
    user_id UUID REFERENCES users(id),
    event_type TEXT NOT NULL,
    detail JSONB NOT NULL DEFAULT '{}',
    prev_hash TEXT,
    row_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Genuinely append-only: revoke UPDATE/DELETE at the database level so no
-- application bug (or compromised app credential) can rewrite history.
-- No RLS here on purpose: this is a platform-global append-only ledger, not
-- tenant-scoped data; tenant_id is nullable attribution, and the policy's
-- tenant context would hide cross-tenant rows from operators.
REVOKE UPDATE, DELETE ON audit_log FROM PUBLIC;
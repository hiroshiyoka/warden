CREATE TABLE egress_rules (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sandbox_id UUID NOT NULL REFERENCES sandboxes(id) ON DELETE CASCADE,
    destination_cidr TEXT NOT NULL,
    destination_port INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- egress_rules hold tenant-scoped data through their parent (sandboxes),
-- so RLS must filter them too — enforced via subquery policy.
ALTER TABLE egress_rules ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation ON egress_rules
    USING (EXISTS (
        SELECT 1 FROM sandboxes s
        WHERE s.id = egress_rules.sandbox_id
          AND s.tenant_id = current_setting('app.current_tenant_id')::uuid
    ))
    WITH CHECK (EXISTS (
        SELECT 1 FROM sandboxes s
        WHERE s.id = egress_rules.sandbox_id
          AND s.tenant_id = current_setting('app.current_tenant_id')::uuid
    ));

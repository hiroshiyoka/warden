# Warden

Security-first PaaS control plane for AI agent execution. **Phase 0** scaffolds the foundation: an Axum API with a real DB-backed health check, Row-Level-Security-gated tenant isolation, and a CI security gate that runs before any build or test.

## Structure

- `warden-api/` — Axum service (Rust). `main.rs` bootstraps tracing, DB pool, migrations, and the router.
- `warden-api/migrations/` — versioned SQL migrations. RLS is enabled in the same migration that creates a table.
- `deny.toml` — cargo-deny policy (advisories deny, license allow-list, ban wildcards).
- `.github/workflows/ci.yml` — `security` job (cargo audit + cargo deny) gates the `build-test` job via `needs:`.

## Local development

```bash
cp .env.example .env
cargo install sqlx-cli --no-default-features --features postgres,rustls
sqlx migrate run --source warden-api/migrations
cargo run -p warden-api
```

Health check: `curl localhost:8080/health` — returns `200 {"status":"ok"}` only if a real `SELECT 1` DB round-trip succeeds.

## Deploying to a small VPS (staging)

Build the image and run it rootless on the VPS. All configuration flows through environment variables — no config files with embedded secrets, so secrets never land on disk or in the image.

```bash
podman build -t warden-api:latest -f Containerfile .
podman run -d --name warden-api \
  -p 8080:8080 \
  -e DATABASE_URL='postgres://<user>:<password>@<vps-ip>:5432/warden' \
  -e RUST_LOG=info \
  warden-api:latest
```

Point `DATABASE_URL` at a Postgres instance (provision one on the same VPS or a managed service such as DigitalOcean Managed Postgres). Keep port `8080` firewalled to only what needs it. No reverse proxy, TLS, or domain routing in this phase — that arrives with the Pingora layer later; for now the raw port behind the firewall is fine.

Podman is used instead of Docker specifically for its rootless, daemonless model: no daemon with host privileges to attack, no privileged deployment user — this is a deliberate security decision, not a preference, and future contributors should not swap it back to Docker. If local Postgres alongside `warden-api` is ever needed, use `podman-compose` or a `podman pod`, not docker-compose against the Podman socket.

## RLS note

Tenant-scoped tables enable Row-Level Security with a policy that matches `app.current_tenant_id` (a per-connection GUC). Phase 1 will set this via `SET LOCAL` inside each authenticated request's transaction. Until then, unauthenticated queries against RLS tables fail closed.

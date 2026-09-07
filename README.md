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

## Secrets (`sops` + `age`)

Any secret that can't be a runtime environment variable lives in `secrets/` encrypted with [sops](https://github.com/getsops/sops) using an [age](https://github.com/FiloSottile/age) keypair. Only `*.enc.yaml` files are committed; the plaintext forms are gitignored (`secrets/*.yaml`).

### First-time setup for a contributor

1. Install `sops` and `age` (e.g. `winget install SecretsOPerationS.SOPS FiloSottile.age`).
2. Generate your own keypair (Windows: sops reads keys from `%APPDATA%\sops\age\keys.txt`; Linux: `~/.config/sops/age/keys.txt` — copy the key wherever your platform expects it):
   ```bash
   age-keygen -o ~/.config/sops/age/keys.txt    # Unix
   # age-keygen -o $env:APPDATA\sops\age\keys.txt   # Windows
   ```
3. Add your public key (`age1...`) to the `age:` list in `secrets/.sops.yaml` so files are encrypted for you too.

### Working with secrets

```bash
sops --decrypt secrets/example.enc.yaml                      # view
sops --encrypt --output secrets/example.enc.yaml secrets/<name>.enc.yaml   # write
```

The private key never enters the repository — keep it in the location above and back it up elsewhere. CI runs `gitleaks` on every push and fails the build if an unencrypted secret pattern (AWS keys, private key headers, etc.) is committed.

## Phase 2A — sandbox foundations without a real runtime

Phase 2A is the slice of Phase 2 that does not require a Linux host with `/dev/kvm` access. It ships the schema, permission-gated API, quota enforcement, audit logging, config validation, and a minimal dashboard — and stops there. The Firecracker runtime is intentionally absent: nothing boots, no kernel is loaded, no VM is created. Phase 2B is the runtime implementation, and it cannot start until a Linux/KVM host is verified.

### What is real today

- `sandboxes` and `egress_rules` tables exist via migrations `0006` and `0007` with Row-Level Security enabled in the same migration that created them.
- `POST /sandboxes` enforces permission, then per-tenant quota (`MAX_ACTIVE_SANDBOXES_PER_TENANT = 5`), and only then attempts to call the runtime. Quota rejection (`429`) and permission rejection (`403`) happen *before* any runtime call.
- `warden-sandbox` defines the `SandboxRuntime` trait and ships exactly one implementation, `UnimplementedRuntime`, which returns `Err(RuntimeError::NotImplemented)` for `boot`, `exec`, and `destroy`. No method can return `Ok(_)` in this phase.
- `ResourceLimits::validate()` and `verify_image_checksum()` are unit-tested for valid and invalid inputs. None of these tests touch a real VM.
- Every request that the API would have booted is recorded with `status = 'pending_runtime'` and an entry in `audit_log` (`sandbox_create_pending_runtime`). The request is real; the boot is not.
- The API surfaces the runtime's honest failure as `503 Service Unavailable` with body `{"error": "sandbox runtime not yet available"}`.

### What the dashboard shows

`apps/web/` is a Vite + React + TanStack Query client that talks to the real API from Task 4. It contains exactly two screens:

- `SandboxList` — renders `GET /sandboxes` and labels any `pending_runtime` row as "Pending runtime — no execution backend yet". The status is not relabelled to something that sounds more finished than it is.
- `SandboxCreate` — submits `POST /sandboxes` and renders the `503` response as "Sandbox request recorded. Execution runtime isn't deployed yet." There is no fake spinner, no auto-retry, no optimistic success state.

No `SandboxLogs` or `SandboxDestroy` screens are built. There is nothing real for them to show, and a UI that pretends otherwise would be worse than no UI.

### Local dev — API + dashboard

```bash
# Terminal 1: API
cargo run -p warden-api

# Terminal 2: dashboard (proxies /api to the running API)
cd apps/web
cp .env.example .env
npm install
npm run dev
```

### Phase 2B gate

Phase 2B (Firecracker runtime, cgroup limits, vsock exec, default-deny networking) only starts after a dated, real `/dev/kvm` check on a Linux host has been recorded in `docs/phase2-host-verification.md`. Without that evidence, no microVM code lands in this repository.

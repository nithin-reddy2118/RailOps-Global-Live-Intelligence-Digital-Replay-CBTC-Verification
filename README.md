# CBTC Platform

A distributed Railway Signaling & Communications-Based Train Control
(CBTC) platform: 15 Rust microservices simulating metro-style train
operations — movement authorities, block occupancy, interlocking safety
logic, signaling, passenger information, maintenance, telemetry,
auditing, and a live React dashboard — built to later host a
deterministic execution-recording/replay layer (PTDF) without
architectural changes.

## Architecture

See `crates/shared/events/src/lib.rs` for the `EventEnvelope`/`DomainEvent`
catalog — the single seam every inter-service interaction (HTTP call,
RabbitMQ message, timer fire) passes through, and the intended future
PTDF interception point.

```
services/
  auth-service                 JWT issuance, users, RBAC
  train-service                Train fleet + telemetry ingestion, heartbeat monitor
  block-occupancy-service      Track block occupancy state machine
  movement-authority-service   Movement authority grant/revoke/expiry lifecycle
  interlocking-service         Route pathfinding + safety conflict checks (the vital core)
  wayside-unit-service         Signal aspect computation, fault simulation
  passenger-info-service       Station/platform arrival information
  maintenance-service          Maintenance ticket lifecycle
  notification-service         Safety-event-driven alerting
  telemetry-service            Historical position-report queries
  audit-log-service            Append-only audit trail (consumes every event)
  comms-gateway                Simulated train<->wayside link, fault injection, blackout control
  central-control-service      Aggregated network overview, emergency-stop-all
  dashboard-backend            WebSocket live event fan-out + snapshot proxy
  api-gateway                  Single external entry point, reverse proxy, rate limiting

crates/shared/                 cbtc-domain, cbtc-errors, cbtc-events, cbtc-db,
                                cbtc-auth, cbtc-observability, cbtc-mq, cbtc-config
crates/testkit/                Shared integration-test harness and fixtures
migrations/                    Single shared Postgres migration set (service-owned schemas)
dashboard-frontend/            Vite/React live control-room UI
infra/docker/                  Parameterized Dockerfile + docker-compose for local dev
infra/k8s/                     Kustomize-based Kubernetes manifests
.github/workflows/ci.yml       Lint, test, and multi-service image build/push
```

## Local development

```bash
cd infra/docker
cp .env.example .env   # fill in INTERLOCKING_SERVICE_TOKEN / CENTRAL_CONTROL_SERVICE_TOKEN — see below
docker compose up --build
```

The gateway listens on `http://localhost:8080`; the dashboard on
`http://localhost:5173`; RabbitMQ management UI on `:15672` (cbtc/cbtc);
Grafana on `:3000` (admin/admin); Prometheus on `:9090`.

Seed login: username `admin`, password `ChangeMe123!` (see
`migrations/20260101000006_seed_data.sql`).

### Service-account tokens

`interlocking-service` and `central-control-service` call
`movement-authority-service` as a service account, not on behalf of an
end user. Mint these once against a running `auth-service`:

```bash
curl -X POST http://localhost:8081/auth/login \
  -H 'content-type: application/json' \
  -d '{"username":"admin","password":"ChangeMe123!"}'
# copy the access_token into .env as INTERLOCKING_SERVICE_TOKEN and
# CENTRAL_CONTROL_SERVICE_TOKEN (a real deployment would mint dedicated
# operator/admin service accounts rather than reusing the seed admin)
```

## Building and testing (run these yourself and paste the output back)

Per the working agreement for this project: I don't fabricate build,
test, or compiler output. Run these locally/in CI and share the actual
results — I'll fix whatever comes back.

```bash
# Full workspace build
cargo build --workspace --all-targets

# Formatting and lints
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Unit tests (no external dependencies)
cargo test --workspace --lib --bins

# Integration tests (require Postgres — see docker-compose's `postgres` service)
export TEST_DATABASE_URL=postgres://cbtc:cbtc@localhost:5432/cbtc_test
cargo test --workspace --tests -- --ignored --test-threads=4
```

```bash
# Dashboard frontend
cd dashboard-frontend && npm ci && npm run build
```

```bash
# Kubernetes manifests (validate without a cluster)
kubectl kustomize infra/k8s | kubectl apply --dry-run=client -f -
```

## Known scope boundaries

Documented here rather than left as silent gaps:

- `sqlx` queries throughout use runtime-checked `query_as::<_, T>(...)`
  rather than the `query!`/`query_as!` macros, so the workspace builds
  without a live database or a committed `.sqlx` offline cache (see the
  note at the top of `services/auth-service/src/repository.rs`).
- Per-client/per-IP rate limiting at `api-gateway` is not implemented —
  only a global request-rate ceiling — since the platform hasn't yet
  defined a client-identity strategy (trains and human operators
  currently share one gateway). Noted in `services/api-gateway/src/router.rs`.
- `passenger-info-service`'s arrival ETA only accounts for a train
  already in the platform's immediate approach block, not a full
  multi-block distance-to-go calculation — adequate for the bundled
  demo topology, flagged as a simplification in `repository.rs`.
- The dashboard's track map lays blocks out alphabetically by name
  rather than by physical chainage, since `TrackBlock` doesn't yet
  carry a chainage field (only `Signal` and `Station` do). Noted in
  `dashboard-frontend/src/TrackMap.tsx`.

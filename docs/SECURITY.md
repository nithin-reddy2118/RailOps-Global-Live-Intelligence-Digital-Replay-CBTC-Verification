# Security Audit

Self-audit against the platform as implemented. Findings are stated
plainly, including gaps — this document exists to be useful, not to
make the project look finished. It has not been reviewed by anyone
other than the model that wrote the code, and should not be treated as
a substitute for an actual third-party security review before any
real deployment.

## Authentication & authorization

- **Password hashing**: Argon2id via the `argon2` crate, random salt
  per password (`crates/shared/auth/src/lib.rs::hash_password`). Verified:
  a unit test (`auth-service/tests/auth_flow.rs::password_hash_is_salted_uniquely`)
  asserts two hashes of the same password differ.
- **JWT**: HS256, secret from `CBTC__AUTH__JWT_SECRET`. **Gap**: HS256
  means every service that validates a token needs the same shared
  secret — compromise of any one service's environment compromises the
  ability to forge tokens platform-wide. A production deployment should
  move to RS256/ES256 with `auth-service` holding the private key and
  every other service holding only the public key.
- **RBAC**: role checks (`AuthenticatedUser::require_role`) are applied
  per-endpoint in every service's handlers. **Gap**: not automatically
  tested — there's no test asserting a `Viewer`-role token is actually
  rejected by every `Operator`+-gated endpoint across all 15 services.
  That coverage should exist before this is trusted in anger.
- **Service-to-service auth**: `interlocking-service` and
  `central-control-service` call `movement-authority-service` with a
  long-lived pre-issued JWT (`INTERLOCKING_SERVICE_TOKEN`,
  `CENTRAL_CONTROL_SERVICE_TOKEN`). **Gap**: these tokens are minted
  once and don't rotate; a real deployment needs short-lived service
  tokens (e.g. minted per-pod from a workload-identity source) instead.

## Input validation

- Every mutating HTTP handler validates its request body via the
  `validator` crate before touching the database (see `req.validate()`
  calls throughout every `handlers.rs`). Malformed input becomes a 422
  `AppError::Validation`, not a panic or an unchecked SQL bind.
- **SQL injection**: not exploitable via any current endpoint — every
  query in every `repository.rs` uses parameterized `sqlx::query`/
  `query_as` with `.bind()`, never string-interpolated user input into
  SQL text. The dynamic-format-string queries that do exist (e.g.
  `format!("SELECT {COLUMNS} FROM ...")`) only ever interpolate
  compile-time constant column lists, never request data.

## Secrets

- No real credentials are committed. `docker-compose.yml` and
  `infra/k8s/config-and-secrets.yaml` both use placeholder values
  explicitly marked as dev-only/not-for-production in comments.
- **Gap**: the Kubernetes `Secret` manifest is committed to the repo
  with placeholder `stringData` — fine for a kind/minikube demo, actively
  wrong for a real cluster. A real deployment must generate that Secret
  out-of-band (Sealed Secrets / External Secrets Operator / cloud KMS),
  never apply the committed manifest as-is.

## CORS / rate limiting / error leakage

- **CORS**: every service currently uses `CorsLayer::permissive()` —
  appropriate for local development, **not** for a production
  deployment, which should restrict `Access-Control-Allow-Origin` to
  the actual dashboard origin.
- **Rate limiting**: `api-gateway` enforces a single global
  requests/second ceiling (`tower::limit::RateLimitLayer`). This is
  explicitly documented as a coarse measure, not per-client throttling
  — see the comment in `services/api-gateway/src/router.rs`. A single
  misbehaving client can still exhaust the shared budget and degrade
  service for everyone else behind the gateway.
- **Error leakage**: `AppError::into_response` (`crates/shared/errors/src/lib.rs`)
  deliberately returns a generic "an internal error occurred" message
  for `Database`, `Internal`, `MessageBroker`, and `Cache` variants,
  while logging the real error server-side with a correlating
  `error_id` the client also receives — so a client can report an
  error_id for operators to look up without the response body ever
  containing a raw SQL error, stack trace, or internal path.

## RabbitMQ / message handling

- Credentials are `cbtc`/`cbtc` in every non-production config
  (docker-compose, the seed k8s ConfigMap) — placeholder only, same
  caveat as the JWT secret and Postgres password above.
- **Ack/nack**: fixed in this pass — see `crates/shared/mq/src/lib.rs`.
  Previously every consumer ACKed unconditionally regardless of handler
  outcome; now success ACKs, a first failure NACKs-with-requeue, and a
  repeated or deserialization failure NACKs to a real dead-letter queue
  (`cbtc.dead-letter`).
- **Gap**: RabbitMQ connections use plain AMQP (`amqp://`), not
  AMQPS/TLS. Acceptable inside a private cluster network; not
  acceptable if RabbitMQ is ever reachable across an untrusted network
  boundary.

## Docker / Kubernetes

- Every service Dockerfile runs as a non-root user (`useradd --system
  --uid 10001`, then `USER cbtc`) — see `infra/docker/Dockerfile.service`.
- **Gap**: Kubernetes manifests set resource `requests`/`limits` but do
  not set a `securityContext` (`readOnlyRootFilesystem`,
  `allowPrivilegeEscalation: false`, `runAsNonRoot: true`,
  dropped capabilities). That should be added before any real cluster
  deployment — noted here rather than silently left off.
- **Gap**: no `NetworkPolicy` resources exist, so by default every pod
  in the `cbtc-platform` namespace can reach every other pod. A real
  deployment should restrict traffic to the documented service-to-service
  calls (e.g. only `interlocking-service` and `central-control-service`
  should be able to reach `movement-authority-service`'s write endpoints).

## Dependency / license posture

See `deny.toml` at the repo root for the enforced policy (license
allowlist, yanked-crate denial, wildcard-dependency denial, unknown
registry/git source denial). `cargo audit` and `cargo deny check` are
both wired into CI (`.github/workflows/ci.yml`'s `dependency-audit`
job) but have not been run against this exact dependency tree from
this sandbox — no network access here to fetch the advisory database.
Run them and treat any real finding as a build-blocking issue, not
something to suppress via `deny.toml`'s `ignore` list without a written
justification.

## Summary

Nothing above is a "todo, ship it anyway" — every gap here is either a
reasonable scope boundary for a research/demo platform (explicitly not
intended to control real infrastructure — see the disclaimer in the
main README) or a concrete next step for anyone taking this toward a
real deployment. Treat this file as a living document: update it as
gaps are closed or new ones are found, don't let it go stale.

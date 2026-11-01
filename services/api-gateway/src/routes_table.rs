//! Static route table: the gateway is a pure reverse proxy, so the
//! table is the entirety of its routing logic. Each entry's `prefix` is
//! matched against the incoming request path (longest-prefix-first);
//! the matched prefix is stripped and the remainder appended to the
//! target service's base URL.

#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub prefix: &'static str,
    pub target_env_var: &'static str,
    pub default_target: &'static str,
}

pub const ROUTES: &[RouteEntry] = &[
    RouteEntry {
        prefix: "/auth",
        target_env_var: "AUTH_SERVICE_URL",
        default_target: "http://auth-service:8080",
    },
    RouteEntry {
        prefix: "/trains",
        target_env_var: "TRAIN_SERVICE_URL",
        default_target: "http://train-service:8080",
    },
    RouteEntry {
        prefix: "/blocks",
        target_env_var: "BLOCK_OCCUPANCY_SERVICE_URL",
        default_target: "http://block-occupancy-service:8080",
    },
    RouteEntry {
        prefix: "/authorities",
        target_env_var: "MOVEMENT_AUTHORITY_SERVICE_URL",
        default_target: "http://movement-authority-service:8080",
    },
    RouteEntry {
        prefix: "/interlocking",
        target_env_var: "INTERLOCKING_SERVICE_URL",
        default_target: "http://interlocking-service:8080",
    },
    RouteEntry {
        prefix: "/wayside",
        target_env_var: "WAYSIDE_UNIT_SERVICE_URL",
        default_target: "http://wayside-unit-service:8080",
    },
    RouteEntry {
        prefix: "/stations",
        target_env_var: "PASSENGER_INFO_SERVICE_URL",
        default_target: "http://passenger-info-service:8080",
    },
    RouteEntry {
        prefix: "/platforms",
        target_env_var: "PASSENGER_INFO_SERVICE_URL",
        default_target: "http://passenger-info-service:8080",
    },
    RouteEntry {
        prefix: "/maintenance",
        target_env_var: "MAINTENANCE_SERVICE_URL",
        default_target: "http://maintenance-service:8080",
    },
    RouteEntry {
        prefix: "/alerts",
        target_env_var: "NOTIFICATION_SERVICE_URL",
        default_target: "http://notification-service:8080",
    },
    RouteEntry {
        prefix: "/telemetry",
        target_env_var: "TELEMETRY_SERVICE_URL",
        default_target: "http://telemetry-service:8080",
    },
    RouteEntry {
        prefix: "/audit",
        target_env_var: "AUDIT_LOG_SERVICE_URL",
        default_target: "http://audit-log-service:8080",
    },
    RouteEntry {
        prefix: "/comms",
        target_env_var: "COMMS_GATEWAY_URL",
        default_target: "http://comms-gateway:8080",
    },
    RouteEntry {
        prefix: "/control",
        target_env_var: "CENTRAL_CONTROL_SERVICE_URL",
        default_target: "http://central-control-service:8080",
    },
    RouteEntry {
        prefix: "/dashboard",
        target_env_var: "DASHBOARD_BACKEND_URL",
        default_target: "http://dashboard-backend:8080",
    },
];

/// Resolves a request path to a target base URL by longest matching
/// prefix, reading each route's override from the environment once at
/// lookup time (cheap: this only runs per-request, not per-byte).
pub fn resolve(path: &str) -> Option<(String, &'static str)> {
    ROUTES
        .iter()
        .filter(|r| path == r.prefix || path.starts_with(&format!("{}/", r.prefix)))
        .max_by_key(|r| r.prefix.len())
        .map(|r| {
            let base =
                std::env::var(r.target_env_var).unwrap_or_else(|_| r.default_target.to_string());
            (base, r.prefix)
        })
}

//! Authentication and authorization primitives shared by every service.
//!
//! `auth-service` is the only service that issues tokens and manages
//! credentials, but every other service needs to *validate* tokens and
//! enforce RBAC on its own endpoints, so those primitives live here
//! rather than requiring a network round-trip to auth-service on every
//! request.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use cbtc_domain::UserId;
use cbtc_errors::AppError;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Roles recognized platform-wide. Individual services may layer
/// finer-grained permission checks on top, but the role enum itself is
/// shared so a token minted by auth-service means the same thing
/// everywhere it's presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full administrative access: user management, topology changes,
    /// fault-injection controls.
    Admin,
    /// Central control room operator: can issue/revoke movement
    /// authorities, acknowledge alerts, command switches.
    Operator,
    /// Maintenance crew: can open/close maintenance tickets, take
    /// blocks out of service.
    Maintainer,
    /// Read-only access to telemetry, dashboard, and passenger info.
    Viewer,
}

impl Role {
    pub fn can_issue_authority(&self) -> bool {
        matches!(self, Role::Admin | Role::Operator)
    }

    pub fn can_manage_maintenance(&self) -> bool {
        matches!(self, Role::Admin | Role::Maintainer)
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Role::Admin => "admin",
            Role::Operator => "operator",
            Role::Maintainer => "maintainer",
            Role::Viewer => "viewer",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject — the authenticated user's ID.
    pub sub: UserId,
    pub roles: Vec<Role>,
    pub iat: i64,
    pub exp: i64,
}

/// Extracted, validated identity attached to a request by the auth
/// middleware / extractor, made available to handlers via
/// `AuthenticatedUser` extraction.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub roles: Vec<Role>,
}

impl AuthenticatedUser {
    pub fn has_role(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }

    pub fn require_role(&self, role: Role) -> Result<(), AppError> {
        if self.has_role(role) || self.has_role(Role::Admin) {
            Ok(())
        } else {
            Err(AppError::forbidden(format!("requires role: {role}")))
        }
    }
}

pub struct JwtCodec {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    ttl_secs: i64,
}

impl JwtCodec {
    pub fn new(secret: &str, ttl_secs: i64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            ttl_secs,
        }
    }

    pub fn issue(&self, user_id: UserId, roles: Vec<Role>) -> Result<String, AppError> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            roles,
            iat: now.timestamp(),
            exp: (now + Duration::seconds(self.ttl_secs)).timestamp(),
        };
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt encode failed: {e}")))
    }

    pub fn validate(&self, token: &str) -> Result<Claims, AppError> {
        let data = decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map_err(|e| AppError::unauthorized(format!("invalid token: {e}")))?;
        Ok(data.claims)
    }
}

/// Hash a plaintext password with Argon2id and a fresh random salt.
pub fn hash_password(plaintext: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hash failed: {e}")))
}

/// Verify a plaintext password against a stored Argon2 hash.
pub fn verify_password(plaintext: &str, stored_hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(stored_hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("stored hash malformed: {e}")))?;
    Ok(Argon2::default()
        .verify_password(plaintext.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Axum extractor: pulls `Authorization: Bearer <token>`, validates it
/// against the `JwtCodec` stored in request extensions (inserted by each
/// service's router setup), and yields an `AuthenticatedUser`.
///
/// Each service registers the codec via `.layer(Extension(jwt_codec))`
/// in its router; this extractor then works identically everywhere.
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let codec = parts
            .extensions
            .get::<std::sync::Arc<JwtCodec>>()
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("JwtCodec not registered on router"))
            })?;

        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("missing Authorization header"))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("Authorization header must be a Bearer token"))?;

        let claims = codec.validate(token)?;
        Ok(AuthenticatedUser {
            user_id: claims.sub,
            roles: claims.roles,
        })
    }
}

/// Marker used by handlers that return a plain 401 without the extractor
/// machinery, e.g. in early health-check-style endpoints.
pub const UNAUTHORIZED_STATUS: StatusCode = StatusCode::UNAUTHORIZED;

use cbtc_auth::Role;
use cbtc_domain::UserId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(length(min = 3, max = 64))]
    pub username: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 12, message = "password must be at least 12 characters"))]
    pub password: String,
    /// Roles to assign at creation. Only an admin caller may set roles
    /// other than `viewer`; enforced in the handler, not here.
    #[serde(default = "default_roles")]
    #[schema(value_type = Vec<String>)]
    pub roles: Vec<Role>,
}

fn default_roles() -> Vec<Role> {
    vec![Role::Viewer]
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    #[schema(value_type = String)]
    pub id: UserId,
    pub username: String,
    pub email: String,
    #[schema(value_type = Vec<String>)]
    pub roles: Vec<Role>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct AssignRoleRequest {
    #[schema(value_type = String)]
    pub role: Role,
}

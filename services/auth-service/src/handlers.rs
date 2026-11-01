use crate::{
    models::{AssignRoleRequest, LoginRequest, RegisterRequest, TokenResponse, UserResponse},
    repository::UserRepository,
    state::AppState,
};
use axum::{
    extract::{Path, State},
    Json,
};
use cbtc_auth::{hash_password, verify_password, AuthenticatedUser, Role};
use cbtc_domain::UserId;
use cbtc_errors::{AppError, AppResult};
use uuid::Uuid;
use validator::Validate;

/// `POST /auth/register` — self-registration always yields `Role::Viewer`
/// regardless of what the request body asks for, unless the caller is
/// already an authenticated admin (checked by a separate admin-only
/// route, `POST /auth/users`, that reuses this handler's core logic via
/// `create_user_internal`).
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<UserResponse>> {
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;
    create_user_internal(&state, req, vec![Role::Viewer])
        .await
        .map(Json)
}

/// `POST /auth/users` — admin-only user creation with arbitrary roles.
pub async fn create_user_as_admin(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<UserResponse>> {
    auth_user.require_role(Role::Admin)?;
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;
    let roles = if req.roles.is_empty() {
        vec![Role::Viewer]
    } else {
        req.roles.clone()
    };
    create_user_internal(&state, req, roles).await.map(Json)
}

async fn create_user_internal(
    state: &AppState,
    req: RegisterRequest,
    roles: Vec<Role>,
) -> AppResult<UserResponse> {
    let password_hash = hash_password(&req.password)?;
    let repo = UserRepository::new(&state.pool);
    let user = repo
        .create_user(&req.username, &req.email, &password_hash)
        .await?;

    for role in &roles {
        repo.assign_role(user.id, *role).await?;
    }

    tracing::info!(user_id = %user.id, username = %user.username, "user created");

    Ok(UserResponse {
        id: UserId::from(user.id),
        username: user.username,
        email: user.email,
        roles,
        active: user.active,
        created_at: user.created_at,
    })
}

/// `POST /auth/login` — verifies credentials and issues a JWT.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<TokenResponse>> {
    req.validate()
        .map_err(|e| AppError::validation(e.to_string()))?;

    let repo = UserRepository::new(&state.pool);
    let user = repo
        .find_by_username(&req.username)
        .await?
        .ok_or_else(|| AppError::unauthorized("invalid username or password"))?;

    if !user.active {
        return Err(AppError::forbidden("account is disabled"));
    }

    let valid = verify_password(&req.password, &user.password_hash)?;
    if !valid {
        return Err(AppError::unauthorized("invalid username or password"));
    }

    let roles = repo.roles_for(user.id).await?;
    let token = state.jwt_codec.issue(UserId::from(user.id), roles)?;

    // Login has no natural home in the `DomainEvent` catalog (it isn't a
    // signaling/train/safety event), so it's recorded directly to the
    // audit log rather than published to RabbitMQ. `event_id` and
    // `causality_id` are still generated here so this row is uniformly
    // correlatable with MQ-sourced audit rows by any downstream tooling
    // (including PTDF) that joins on those columns.
    sqlx::query(
        r#"INSERT INTO audit.log_entries (event_id, causality_id, origin_service, actor_user_id, action, details)
           VALUES ($1, $2, 'auth-service', $3, 'user_login', '{}'::jsonb)"#,
    )
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in: 3600,
    }))
}

/// `GET /auth/me` — returns the caller's own profile.
pub async fn me(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<UserResponse>> {
    let repo = UserRepository::new(&state.pool);
    let (user, roles) = repo.get_with_roles(auth_user.user_id).await?;
    Ok(Json(UserResponse {
        id: UserId::from(user.id),
        username: user.username,
        email: user.email,
        roles,
        active: user.active,
        created_at: user.created_at,
    }))
}

/// `POST /auth/users/:id/roles` — admin-only role assignment.
pub async fn assign_role(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(req): Json<AssignRoleRequest>,
) -> AppResult<Json<UserResponse>> {
    auth_user.require_role(Role::Admin)?;

    let repo = UserRepository::new(&state.pool);
    repo.assign_role(user_id, req.role).await?;
    let (user, roles) = repo.get_with_roles(UserId::from(user_id)).await?;

    Ok(Json(UserResponse {
        id: UserId::from(user.id),
        username: user.username,
        email: user.email,
        roles,
        active: user.active,
        created_at: user.created_at,
    }))
}

//! User repository. Deliberately uses `sqlx::query_as::<_, T>(...)` with
//! runtime query strings rather than the `query!`/`query_as!` macros:
//! those macros require a live database (or a committed `.sqlx` cache)
//! at *compile time*, which would couple every contributor's build to a
//! running Postgres instance. Runtime-checked queries trade compile-time
//! SQL verification for that independence — a deliberate call for a
//! portfolio project meant to build cleanly out of the box.

use cbtc_auth::Role;
use cbtc_domain::UserId;
use cbtc_errors::{AppError, AppResult};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

pub struct UserRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> UserRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_user(
        &self,
        username: &str,
        email: &str,
        password_hash: &str,
    ) -> AppResult<UserRow> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO auth.users (username, email, password_hash)
            VALUES ($1, $2, $3)
            RETURNING id, username, email, password_hash, active, created_at
            "#,
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .fetch_one(self.pool)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                AppError::conflict("username or email already registered")
            }
            _ => AppError::from(e),
        })?;

        Ok(row)
    }

    pub async fn assign_role(&self, user_id: Uuid, role: Role) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO auth.user_roles (user_id, role)
            VALUES ($1, $2::user_role)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(role_to_db_str(role))
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_username(&self, username: &str) -> AppResult<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, email, password_hash, active, created_at
            FROM auth.users
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(&self, user_id: Uuid) -> AppResult<Option<UserRow>> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, username, email, password_hash, active, created_at
            FROM auth.users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool)
        .await?;
        Ok(row)
    }

    pub async fn roles_for(&self, user_id: Uuid) -> AppResult<Vec<Role>> {
        let rows: Vec<(String,)> =
            sqlx::query_as(r#"SELECT role::text FROM auth.user_roles WHERE user_id = $1"#)
                .bind(user_id)
                .fetch_all(self.pool)
                .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(r,)| role_from_db_str(&r))
            .collect())
    }

    pub async fn get_with_roles(&self, user_id: UserId) -> AppResult<(UserRow, Vec<Role>)> {
        let user = self
            .find_by_id(user_id.as_uuid())
            .await?
            .ok_or_else(|| AppError::not_found("user", user_id.to_string()))?;
        let roles = self.roles_for(user.id).await?;
        Ok((user, roles))
    }
}

fn role_to_db_str(role: Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::Operator => "operator",
        Role::Maintainer => "maintainer",
        Role::Viewer => "viewer",
    }
}

fn role_from_db_str(s: &str) -> Option<Role> {
    match s {
        "admin" => Some(Role::Admin),
        "operator" => Some(Role::Operator),
        "maintainer" => Some(Role::Maintainer),
        "viewer" => Some(Role::Viewer),
        _ => None,
    }
}

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{
    generate_refresh_token, hash_refresh_token, issue_access_token, verify_password,
};
use crate::db;
use crate::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let user = db::users::find_by_email(&state.db, &body.email)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if user.deactivated_at.is_some() {
        return Err(ApiError::Unauthorized);
    }

    let password_hash = user
        .password_hash
        .as_deref()
        .ok_or(ApiError::Unauthorized)?;

    if !verify_password(&body.password, password_hash) {
        return Err(ApiError::Unauthorized);
    }

    let access_token = issue_access_token(&state.config, user.id, &user.role)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let token_hash = hash_refresh_token(&raw_refresh);

    db::refresh_tokens::create(
        &state.db,
        user.id,
        &token_hash,
        state.config.refresh_expiry_seconds,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: raw_refresh,
        token_type: "Bearer".into(),
        expires_in: state.config.jwt_expiry_seconds,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let token_hash = hash_refresh_token(&body.refresh_token);

    let token_row = db::refresh_tokens::find_valid(&state.db, &token_hash)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    // Delete the used refresh token (rotation)
    db::refresh_tokens::delete(&state.db, token_row.id).await?;

    let user = db::users::find_by_id(&state.db, token_row.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if user.deactivated_at.is_some() {
        return Err(ApiError::Unauthorized);
    }

    let access_token = issue_access_token(&state.config, user.id, &user.role)
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let raw_refresh = generate_refresh_token();
    let new_hash = hash_refresh_token(&raw_refresh);

    db::refresh_tokens::create(
        &state.db,
        user.id,
        &new_hash,
        state.config.refresh_expiry_seconds,
    )
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: raw_refresh,
        token_type: "Bearer".into(),
        expires_in: state.config.jwt_expiry_seconds,
    }))
}

/// JIT provisioning: create or update a user from Barbacane headers.
pub async fn jit_provision(
    pool: &sqlx::PgPool,
    external_id: &str,
    username: &str,
    display_name: &str,
    email: Option<&str>,
) -> Result<Uuid, ApiError> {
    if let Some(user) = db::users::find_by_external_id(pool, external_id).await? {
        return Ok(user.id);
    }

    let id = burst_core::id::new_id();
    db::users::create(
        pool,
        &db::users::CreateUser {
            id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            email: email.map(String::from),
            password_hash: None,
            external_id: Some(external_id.to_string()),
            role: "member".to_string(),
        },
    )
    .await?;

    Ok(id)
}

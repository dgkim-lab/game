use crate::database::Database;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post},
    Json, Router,
};
use rand_core::OsRng;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SESSION_TTL_SECONDS: u64 = 86_400;

#[derive(Clone)]
pub struct AuthState {
    pub database: Database,
    pub redis: redis::Client,
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub account_id: i64,
    pub character_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: &'static str,
}

pub async fn run_api(address: &str, state: AuthState) -> std::io::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/session", delete(logout))
        .layer(middleware::from_fn(request_logging))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, app).await
}

async fn health() -> &'static str {
    "ok"
}

async fn signup(
    State(state): State<AuthState>,
    Json(credentials): Json<Credentials>,
) -> Result<(StatusCode, Json<AuthResponse>), (StatusCode, Json<ErrorResponse>)> {
    validate_credentials(&credentials)?;
    let password_hash = hash_password(&credentials.password).map_err(internal_error)?;
    let account = state
        .database
        .create_account(&credentials.username, &password_hash)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| conflict("username already exists"))?;
    let token = create_session(&state.redis, account.account_id)
        .await
        .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            account_id: account.account_id,
            character_id: account.character_id,
        }),
    ))
}

async fn login(
    State(state): State<AuthState>,
    Json(credentials): Json<Credentials>,
) -> Result<Json<AuthResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_credentials(&credentials)?;
    let account = state
        .database
        .find_account(&credentials.username)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| unauthorized("invalid username or password"))?;
    verify_password(&credentials.password, &account.password_hash)
        .map_err(|_| unauthorized("invalid username or password"))?;
    let character_id = state
        .database
        .character_for_account(account.account_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| internal_error("account has no character"))?;
    let token = create_session(&state.redis, account.account_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(AuthResponse {
        token,
        account_id: account.account_id,
        character_id,
    }))
}

async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let token = bearer_token(&headers).ok_or_else(|| unauthorized("missing bearer token"))?;
    let mut connection = state
        .redis
        .get_multiplexed_async_connection()
        .await
        .map_err(internal_error)?;
    let _: usize = connection
        .del(session_key(token))
        .await
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_session(
    redis: &redis::Client,
    account_id: i64,
) -> Result<String, redis::RedisError> {
    let token = Uuid::new_v4().to_string();
    let mut connection = redis.get_multiplexed_async_connection().await?;
    let _: () = connection
        .set_ex(session_key(&token), account_id, SESSION_TTL_SECONDS)
        .await?;
    Ok(token)
}

fn session_key(token: &str) -> String {
    format!("frontier:session:{token}")
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<(), argon2::password_hash::Error> {
    let parsed_hash = PasswordHash::new(hash)?;
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash)
}

fn validate_credentials(
    credentials: &Credentials,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if !(3..=32).contains(&credentials.username.len()) {
        return Err(bad_request("username must be 3-32 characters"));
    }
    if credentials.password.len() < 8 {
        return Err(bad_request("password must be at least 8 characters"));
    }
    Ok(())
}

async fn request_logging(request: Request, next: Next) -> Response {
    tracing::info!(method = %request.method(), path = %request.uri().path(), "auth request");
    next.run(request).await
}

fn bad_request(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
}

fn unauthorized(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse { error: message }),
    )
}

fn conflict(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::CONFLICT, Json(ErrorResponse { error: message }))
}

fn internal_error<E>(_error: E) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal server error",
        }),
    )
}

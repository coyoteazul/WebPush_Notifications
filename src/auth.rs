use std::sync::Arc;

use axum::{extract::{Request, State}, http::StatusCode, middleware::Next, response::Response};
use tracing::info;

pub async fn auth(
    State(api_key): State<Arc<String>>,
    req: Request, 
    next: Next
) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get("api_key")
        .and_then(|header| header.to_str().ok());

    let auth_header = if let Some(auth_header) = auth_header {
        auth_header
    } else {
        info!("StatusCode::UNAUTHORIZED Missing api_key header");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if auth_header == *api_key {
        // If the API key matches, proceed to the next handler
        Ok(next.run(req).await)
    } else {
        // Otherwise, return Unauthorized
        info!("StatusCode::UNAUTHORIZED api_key header doesn't match");
        Err(StatusCode::UNAUTHORIZED)
    }
}
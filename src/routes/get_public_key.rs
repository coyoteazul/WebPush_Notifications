use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::conf::KeysJson;

#[derive(utoipa::IntoResponses,Deserialize,Serialize, ToSchema)]
pub enum GetPuKeyResponses {
    /// Success response
    #[response(status = 200)]
    Ok(String),
}

impl IntoResponse for GetPuKeyResponses {
    fn into_response(self) -> axum::response::Response {
        match self {
            GetPuKeyResponses::Ok(msg) => (StatusCode::OK, msg).into_response(),
        }
    }
}

#[utoipa::path(get, path = "/get_public_key", responses(GetPuKeyResponses))]
pub async fn get_public_key(
    State(conf): State<Arc<KeysJson>>,
) -> GetPuKeyResponses {
    return GetPuKeyResponses::Ok(conf.public_key.clone());
}
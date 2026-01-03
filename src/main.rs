use std::sync::Arc;
use axum::{Json, middleware};
use utoipa_axum::router::OpenApiRouter;
use crate::{auth::auth, conf::{ConfFile, load_conf_file}, routes::{get_public_key::*, notify::*}};


pub mod auth;
pub mod conf;
pub mod routes;

#[tokio::main]
async fn main() {

    let ConfFile { openapi, keys, server } = load_conf_file();
    
    tracing_subscriber::fmt()
    .with_max_level(server.trace_level)
    .init();

    let state = Arc::new(keys);
    let api_key = Arc::new(server.api_key);
        
    //Armar rutas y openapi
    let (mut router, mut api): (axum::Router, utoipa::openapi::OpenApi) = OpenApiRouter::new()
        .routes(utoipa_axum::routes!(get_public_key))
        .routes(utoipa_axum::routes!(notify))
        .with_state(state)
        .route_layer(middleware::from_fn_with_state(api_key, auth))
        .split_for_parts();
    
    //Trasladar configuracion a openapi
    api.info.contact     = Some(openapi.contact);
    api.info.title       = openapi.title;
    api.info.description = Some(openapi.description);
    api.info.version     = openapi.version;
    
    //agregar url de openapi a las rutas
    router = router
        .route("/openapi.json", axum::routing::get(Json(api)));


    let listener = tokio::net::TcpListener::bind(format!("{}:{}",server.accept_from, server.port)).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

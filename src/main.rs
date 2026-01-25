use std::sync::Arc;
use axum::{Json, middleware};
use tracing::{debug, trace};
use utoipa_axum::router::OpenApiRouter;
use crate::{auth::auth, conf::{ConfFile, load_conf_file}, routes::{get_public_key::*, notify::*}};


pub mod auth;
pub mod conf;
pub mod routes;

#[cfg(windows)]
mod windows_service;

fn init_tokio(router: axum::Router, addr: String) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            run_server(router, addr).await
        })
}


async fn run_server(router: axum::Router, addr: String) -> anyhow::Result<()> {
    trace!("Starting axum server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
    .await?;

    debug!("Axum server stopped.");

    Ok(())
}

fn init_server() -> (axum::Router, String) {
    let ConfFile { openapi, keys, server } = load_conf_file();
    
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

    let addr:String = format!("{}:{}",server.accept_from, server.port);

    (router, addr)
}




fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().collect();

        if args.contains(&"--help".into()) {
            print_help();
            return Ok(());
        }

        if args.contains(&"--install".into()) {
            windows_service::install()?;
            println!("Service installed successfully.");
            return Ok(());
        }

        if args.contains(&"--uninstall".into()) {
            windows_service::uninstall()?;
            println!("Service removed successfully.");
            return Ok(());
        }

        if args.contains(&"--console".into()) {
            let (router, addr) = init_server();
            return init_tokio(router, addr);
        }

        // Started by SCM (no args)
        return windows_service::run();
    }

    #[cfg(not(windows))] {
        let (router, addr) = init_server();
        return init_tokio(router, addr);
    }
}


fn print_help() {
    println!(
        r#"
WebPush backend service

USAGE:
  webpush.exe --install     Install Windows service
  webpush.exe --uninstall   Remove Windows service
  webpush.exe --console     Run in console mode
  webpush.exe --help        Show this help

No arguments will show this message.
"#
    );
}

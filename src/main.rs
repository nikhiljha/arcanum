#![allow(unused_imports, unused_variables)]
pub use arcanum::*;
use prometheus::{Encoder, TextEncoder};
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

use actix_web::{
    get, middleware,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use ecies_ed25519::{PublicKey, SecretKey};

#[get("/metrics")]
async fn metrics(c: Data<Manager>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().body(buffer)
}

#[get("/pubkey")]
async fn pubkey(c: Data<Manager>, _req: HttpRequest) -> impl Responder {
    let sk =
        SecretKey::from_bytes(&*base64::decode(std::env::var("ARCANUM_ENC_KEY").unwrap()).unwrap())
            .unwrap();
    let pk = PublicKey::from_secret(&sk);
    HttpResponse::Ok().body(base64::encode(pk.as_bytes()))
}

#[get("/health")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/")]
async fn index(c: Data<Manager>, _req: HttpRequest) -> impl Responder {
    let state = c.state().await;
    HttpResponse::Ok().json(&state)
}

#[tokio::main]
async fn main() -> Result<()> {
    let logger = tracing_subscriber::fmt::layer().json();
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let collector = Registry::default().with(logger).with(env_filter);

    // Initialize tracing
    tracing::subscriber::set_global_default(collector).unwrap();

    // Start kubernetes controller
    let (manager, drainer) = Manager::new().await;

    // Start web server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(manager.clone()))
            .wrap(middleware::Logger::default().exclude("/health"))
            .service(index)
            .service(health)
            .service(metrics)
            .service(pubkey)
    })
    .bind("0.0.0.0:8080")
    .expect("bind to 0.0.0.0:8080")
    .shutdown_timeout(5);

    tokio::select! {
        _ = drainer => warn!("controller drained"),
        _ = server.run() => info!("actix exited"),
    }
    Ok(())
}

use actix_web::{get, web, App, HttpServer, Responder, Result};
use log::{debug, error};
use query::{ChannelNode, StatusCache, CACHE_LIFETIME};
use serde::Serialize;
use std::{
    env,
    ops::Sub,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

mod query;

#[derive(Clone, Debug)]
pub struct Config {
    ts3_host: String,
    ts3_port: u16,
    ts3_server_id: u64,
    user: String,
    password: String,
}

#[derive(Clone)]
pub struct State {
    cfg: Config,
    cache: Arc<RwLock<StatusCache>>,
}

#[derive(Serialize)]
pub struct JsonResponse {
    pub success: bool,
    pub error: Option<String>,
    pub channels: Option<ChannelNode>,
}

#[get("/")]
async fn status(state: web::Data<State>) -> Result<impl Responder> {
    debug!("status: {:?}", state.cfg);
    let result = query::fetch_status(&state.cfg, &state.cache).await;

    if let Err(e) = result.as_ref() {
        error!("TS3 Error: {:?}", e);
    }

    let response = JsonResponse {
        success: result.is_ok(),
        error: result.as_ref().map_err(|e| format!("{:?}", e)).err(),
        channels: result.ok(),
    };

    Ok(web::Json(response))
}

fn build_state() -> State {
    let cfg = Config {
        ts3_host: env::var("TS3_HOST").expect("TS3_HOST not set"),
        ts3_port: env::var("TS3_PORT")
            .expect("TS3_PORT not set")
            .parse()
            .expect("invalid port"),
        ts3_server_id: env::var("TS3_SERVER_ID")
            .expect("TS3_SERVER_ID not set")
            .parse()
            .expect("invalid server id"),
        user: env::var("TS3_USER").expect("TS3_USER not set"),
        password: env::var("TS3_PASS").expect("TS3_PASS not set"),
    };

    let cache = Arc::new(RwLock::new(StatusCache {
        last_update: Instant::now().sub(Duration::from_secs(CACHE_LIFETIME)),
        root: ChannelNode::default(),
    }));

    State { cfg, cache }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let listen = args.get(1).expect("Listening address:port not specified");

    let state = build_state();
    HttpServer::new(move || App::new().data(state.clone()).service(status))
        .bind(listen)?
        .run()
        .await
}

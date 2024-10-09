use actix_web::{web, App, HttpServer};
use config::Environment;
use hnsw_rs::prelude::*;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::io::Result as IoResult;
use std::path::Path;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::Mutex;
use tracing::Level;
use tracing_subscriber::fmt::SubscriberBuilder;
use tracing_subscriber::EnvFilter;

mod endpoints;

const SIMILARITY_THRESHOLD: f32 = 5.0;

#[derive(Debug, Deserialize)]
struct Config {
    database_url: String,
    port: String,
}

#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
struct DistL1;

impl Distance<u8> for DistL1 {
    fn eval(&self, va: &[u8], vb: &[u8]) -> f32 {
        va.iter()
            .zip(vb.iter())
            .map(|(x, y)| (f32::from(*x) - f32::from(*y)).abs())
            .sum()
    }
}

#[derive(Clone)]
struct AppState {
    hnsw: Arc<Mutex<Hnsw<'static, u8, DistL1>>>,
}

async fn get_db_pool(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(100)
        .connect(database_url)
        .await
}

async fn save_hnsw_tree(hnsw: Arc<Mutex<Hnsw<'_, u8, DistL1>>>, path: &Path) {
    let hnsw = hnsw.lock().await;
    let binding = "data_dump".to_owned();
    if let Err(err) = hnsw.file_dump(&binding) {
        println!("Failed to save HNSW tree: {err:?}");
    } else {
        println!("HNSW tree saved to {}", path.to_str().unwrap_or_default());
    }
}

fn load_hnsw_tree(path: &Path) -> Option<Hnsw<'static, u8, DistL1>> {
    let options = ReloadOptions::default().set_mmap(true);
    let binding = "data_dump".to_owned();
    let mut loader = HnswIo::new_with_options(path.to_path_buf(), binding, options);

    let x = loader.load_hnsw::<u8, DistL1>().map_or_else(
        |_| {
            println!("Failed to load HNSW tree");
            None
        },
        |hnsw| unsafe {
            let hnsw_static: Hnsw<'static, u8, DistL1> = std::mem::transmute(hnsw);
            Some(hnsw_static)
        },
    );
    x
}

#[actix_web::main]
async fn main() -> IoResult<()> {
    let settings = config::Config::builder()
        .add_source(Environment::with_prefix("APP"))
        .build()
        .expect("Failed to configure Environment Settings");
    let config: Config = settings
        .try_deserialize()
        .expect("Failed to deserialize Environment Configuration");

    let filter = EnvFilter::from_default_env()
        .add_directive(Level::INFO.into())
        .add_directive(
            "tracing::span=off"
                .parse()
                .expect("Failed to set logger's filter parameters"),
        );
    SubscriberBuilder::default().with_env_filter(filter).init();

    let app_ip = format!("0.0.0.0:{}", config.port);
    tracing::info!("Starting service at {}", app_ip);

    let db_pool = get_db_pool(&config.database_url)
        .await
        .expect("Failed to get a connection to the Database");

    let max_nb_connection = 16;
    let max_elements = 10_000;
    let ef_construction = 200;

    let hnsw_file_path = Path::new("hnsw_tree.dump");

    let hnsw = load_hnsw_tree(hnsw_file_path).map_or_else(
        || {
            println!("Creating a new HNSW tree.");
            Hnsw::<u8, DistL1>::new(
                max_nb_connection,
                max_elements,
                max_elements,
                ef_construction,
                DistL1 {},
            )
        },
        |loaded_hnsw| {
            println!("HNSW tree loaded from {}", hnsw_file_path.display());
            loaded_hnsw
        },
    );

    let shared_state = AppState {
        hnsw: Arc::new(Mutex::new(hnsw)),
    };

    let cloned_shared_state = shared_state.clone();

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(shared_state.clone()))
            .app_data(web::Data::new(db_pool.clone()))
            .configure(endpoints::configure)
    })
    .bind(app_ip)?
    .run();

    let signal_task = tokio::spawn(async move {
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
        sigint.recv().await;

        println!("SIGINT received. Saving HNSW tree...");

        save_hnsw_tree(cloned_shared_state.hnsw, hnsw_file_path).await;
        println!("HNSW tree saved successfully.");

        std::process::exit(0);
    });

    tokio::select! {
        _ = server => (),
        _ = signal_task => (),
    }

    Ok(())
}

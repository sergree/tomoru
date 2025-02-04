use anyhow::{Context, Result};
use axum::{
    extract::ConnectInfo,
    extract::{Request, State},
    middleware::{from_fn_with_state, Next},
    response::Response,
    routing::get,
    Router,
};
use std::net::IpAddr;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time;

// Stores request statistics for the application
// Note: For production use, consider using DashMap or external storage
#[derive(Default)]
struct AppState {
    ip_counts: HashMap<IpAddr, u64>,
}

/// Tracks request count per IP address and forwards the request
async fn counter_middleware(
    State(app_state): State<Arc<Mutex<AppState>>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    {
        let mut stats = app_state
            .lock()
            .map_err(|e| eprintln!("Lock poisoned in middleware: {}", e))
            .expect("Failed to acquire lock");

        *stats.ip_counts.entry(addr.ip()).or_default() += 1;
    }
    next.run(request).await
}

/// Basic /ping endpoint
async fn ping() -> &'static str {
    "pong"
}

/// Prints current request statistics every second
async fn print_stats(stats: Arc<Mutex<AppState>>) -> Result<()> {
    let mut interval = time::interval(Duration::from_secs(1));

    loop {
        interval.tick().await;

        let stats = stats
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock poisoned in print_stats: {}", e))?;

        // Collect and sort IP counts here in the print function since it (usually) runs less frequently
        // than the counter_middleware(), optimizing overall performance
        let mut counts: Vec<_> = stats.ip_counts.iter().collect();
        counts.sort_by(|(_, a), (_, b)| b.cmp(a));

        println!("IPs:");
        for (ip, count) in counts {
            println!("  {}: {}", ip, count);
        }
        println!();
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize shared application state
    // Note: This is a simplified approach and might not be suitable for production
    let stats: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::default()));
    let stats_clone = stats.clone();

    // Start the background task for printing statistics
    tokio::spawn(async move {
        if let Err(e) = print_stats(stats_clone).await {
            eprintln!("Stats printer error: {:#}", e);
        }
    });

    // Set up the application routes and middleware
    let app = Router::new()
        .route("/ping", get(ping))
        .layer(from_fn_with_state(stats.clone(), counter_middleware))
        .with_state(stats);

    // Start the server on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .context("Failed to bind to port 3000")?;

    println!("Server running on http://0.0.0.0:3000");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("Server error")?;

    Ok(())
}

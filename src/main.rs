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

impl AppState {
    // Increment IP count
    fn increment_ip_count(&mut self, ip: IpAddr) {
        *self.ip_counts.entry(ip).or_default() += 1;
    }

    // Get sorted IP counts
    fn get_sorted_ip_counts(&self) -> Vec<(IpAddr, u64)> {
        // Collect and sort IP counts here since it (usually) runs less frequently
        // than the increment_ip_count(), optimizing overall performance
        let mut counts: Vec<_> = self
            .ip_counts
            .iter()
            .map(|(ip, count)| (*ip, *count))
            .collect();
        counts.sort_by(|(_, a), (_, b)| b.cmp(a));
        counts
    }

    // Format IP statistics
    fn format_ip_stats(&self) -> String {
        let counts = self.get_sorted_ip_counts();
        let mut result = String::from("IPs:\n");
        for (ip, count) in counts {
            result.push_str(&format!("  {}: {}\n", ip, count));
        }
        result
    }
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

        stats.increment_ip_count(addr.ip());
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

        println!("{}", stats.format_ip_stats());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn increment_ip_count() {
        let mut state = AppState::default();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        state.increment_ip_count(ip);
        assert_eq!(*state.ip_counts.get(&ip).unwrap(), 1);

        state.increment_ip_count(ip);
        assert_eq!(*state.ip_counts.get(&ip).unwrap(), 2);
    }

    #[test]
    fn get_sorted_ip_counts() {
        let mut state = AppState::default();
        let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));

        state.increment_ip_count(ip1);
        state.increment_ip_count(ip1);
        state.increment_ip_count(ip2);

        let sorted = state.get_sorted_ip_counts();
        assert_eq!(sorted, vec![(ip1, 2), (ip2, 1)]);
    }

    #[test]
    fn format_ip_stats() {
        let mut state = AppState::default();
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        state.increment_ip_count(ip);

        let formatted = state.format_ip_stats();
        let expected = format!("IPs:\n  {}: 1\n", ip);
        assert_eq!(formatted, expected);
    }
}

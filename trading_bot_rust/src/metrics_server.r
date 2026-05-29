use warp::Filter;
use prometheus::{Encoder, TextEncoder};

pub async fn start_metrics_server() {
    let metrics_route = warp::path("metrics").map(|| {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    });
    
    warp::serve(metrics_route).run(([0, 0, 0, 0], 8080)).await;
}
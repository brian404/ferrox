use hyper::{Body, Request, Response, StatusCode};
use hyper::body::Incoming;
use std::time::Instant;
use tracing::{error, info};

pub async fn logged_route(req: Request<Incoming>) -> Result<Response<Body>, hyper::Error> {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Call the real router
    let mut response = match crate::router::route(req).await {
        Ok(resp) => resp.map(|b| Body::wrap_stream(b.into())),
        Err(e) => {
            error!("Handler error: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal Server Error"))
                .unwrap()
        }
    };

    // Add common security headers
    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());

    // Log the outcome
    let duration = start.elapsed();
    let status = response.status();
    info!(
        "{} {} → {} ({:?}, {} bytes)",
        method,
        path,
        status,
        duration,
        response.size_hint().upper().unwrap_or(0)
    );

    Ok(response)
}

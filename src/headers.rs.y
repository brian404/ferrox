use hyper::{HeaderMap, Response};
use hyper::header;

pub fn add_common_headers<B>(mut response: Response<B>) -> Response<B> {
    let headers = response.headers_mut();

    headers.insert("Server", "ferrox".parse().unwrap());
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());

    response
}

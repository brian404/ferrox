use anyhow::Result;
use hyper::body::Incoming;
use hyper::{Request, Response};

use crate::body::{full_body, ResponseBody};

pub async fn handle_health(_req: Request<Incoming>) -> Result<Response<ResponseBody>, hyper::Error> {
    let response = Response::new(full_body("OK - Ferrox is healthy"));
    Ok(crate::headers::add_common_headers(response))
}

mod common;

use common::start_test_server_custom;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

#[tokio::test]
async fn stalled_connection_is_closed_after_read_timeout() {
    let port = start_test_server_custom("127.0.0.1:1", Duration::from_millis(300)).await;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    // Deliberately send nothing at all.

    tokio::time::sleep(Duration::from_millis(600)).await;

    let mut buf = [0u8; 16];
    let result = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut buf)).await;

    match result {
        Ok(Ok(0)) => {} // EOF - server closed the connection, as expected
        Ok(Ok(n)) => panic!("unexpected {} bytes read; expected EOF", n),
        Ok(Err(_)) => {} // a reset is also acceptable evidence the server closed it
        Err(_) => panic!("stream was never closed - the read-timeout did not fire"),
    }
}

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

#[tokio::test]
async fn parse_ok_reply() {
    let reply = "HELLO REPLY RESULT=OK VERSION=3.3";
    let parsed = parse_hello_reply(reply).unwrap();
    assert_eq!(parsed.result, "OK");
    assert_eq!(parsed.version.as_deref(), Some("3.3"));
}

#[tokio::test]
async fn hello_version_roundtrip_over_duplex() {
    let (mut client, mut server) = duplex(1024);

    // Fake server: read command, respond with HELLO REPLY
    let server_task = tokio::spawn(async move {
        let mut buf = [0u8; 256];
        let n = server.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[..n]).to_string();

        assert!(received.starts_with("HELLO VERSION"));
        assert!(received.contains("MIN=3.1"));
        assert!(received.contains("MAX=3.3"));

        server
            .write_all(b"HELLO REPLY RESULT=OK VERSION=3.3\n")
            .await
            .unwrap();
    });

    let res = hello_version(&mut client, SamHelloRequest::default()).await.unwrap();
    assert_eq!(res.result, "OK");
    assert_eq!(res.version.as_deref(), Some("3.3"));

    server_task.await.unwrap();
}

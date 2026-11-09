use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

fn storage(endpoint: Option<String>) -> S3AttachmentStorage {
    let mut config = aws_sdk_s3::Config::builder()
        .behavior_version(aws_config::BehaviorVersion::latest())
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            "test",
            "test",
            Some("test-session-token".into()),
            None,
            "test",
        ))
        .force_path_style(true);
    if let Some(endpoint) = endpoint {
        config = config.endpoint_url(endpoint);
    }
    S3AttachmentStorage::new(Client::from_conf(config.build()), "private-mail".into())
}

fn request() -> CreateAttachmentUpload {
    CreateAttachmentUpload {
        filename: "notes.txt".into(),
        content_type: "text/plain".into(),
        size_bytes: 4,
        checksum_sha256: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
    }
}

fn server(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                socket.write_all(response.as_bytes()).unwrap();
                String::from_utf8(bytes).unwrap()
            })
            .collect()
    });
    (address, worker)
}

fn head(size: usize) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {size}\r\ncontent-type: text/plain\r\nx-amz-checksum-sha256: {}\r\nx-amz-version-id: source-version\r\nconnection: close\r\n\r\n",
        request().checksum_sha256
    )
}

#[tokio::test]
async fn signed_upload_constrains_length_checksum_and_expires() {
    let grant = storage(None)
        .grant_upload("attachments/staging/test", &request())
        .await
        .unwrap();
    assert!(
        grant.url.starts_with(
            "https://s3.us-east-1.amazonaws.com/private-mail/attachments/staging/test?"
        )
    );
    assert!(grant.url.contains("X-Amz-Expires=900"));
    assert!(grant.url.contains("content-length"));
    assert!(
        grant
            .url
            .to_ascii_lowercase()
            .contains("x-amz-checksum-sha256")
    );
    let allowed = [
        "content-type",
        "x-amz-checksum-sha256",
        "x-amz-sdk-checksum-algorithm",
        "x-amz-content-sha256",
        "x-amzn-trace-id",
    ];
    for name in grant.headers.keys() {
        assert!(
            allowed.contains(&name.as_str()),
            "browser upload requires unexpected header {name}"
        );
    }
    assert!(
        !grant
            .headers
            .keys()
            .any(|key| key.eq_ignore_ascii_case("authorization")
                || key.eq_ignore_ascii_case("content-length"))
    );
}

#[tokio::test]
async fn finalization_rejects_a_different_size_before_copying() {
    let (endpoint, worker) = server(vec![head(5)]);
    let error = storage(Some(endpoint))
        .finalize_upload("attachments/staging/test", &request())
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Validation(_)));
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("HEAD "));
}

#[tokio::test]
async fn finalization_copies_the_verified_version_and_downloads_force_attachment() {
    let body = "<CopyObjectResult><ETag>test</ETag><LastModified>2026-01-01T00:00:00Z</LastModified></CopyObjectResult>";
    let copy = format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nx-amz-version-id: final-version\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let (endpoint, worker) = server(vec![head(4), copy]);
    let storage = storage(Some(endpoint));
    let object = storage
        .finalize_upload("attachments/staging/test", &request())
        .await
        .unwrap();
    assert_eq!(object.version_id, "final-version");
    assert!(object.key.starts_with("attachments/files/"));
    let requests = worker.join().unwrap();
    assert!(requests[1].contains("versionId=source-version"));
    let signed = storage.download(&object, "../notes.txt").await.unwrap();
    assert!(signed.url.contains("versionId=final-version"));
    assert!(
        signed
            .url
            .contains("response-content-disposition=attachment")
    );
    assert!(signed.url.contains("application%2Foctet-stream"));
}

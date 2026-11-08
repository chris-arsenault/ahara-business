use super::{ApiState, StatusCode, authenticated_request_with_state, response_json};
use serde_json::json;

#[tokio::test]
async fn reply_with_fifteen_mib_attachment_uses_only_a_reference_in_the_api() {
    let state = ApiState::for_tests();
    let response = authenticated_request_with_state(
        state.clone(),
        "POST",
        "/attachments/uploads",
        Some(json!({
            "filename": "notes.txt", "content_type": "text/plain", "size_bytes": 15 * 1024 * 1024,
            "checksum_sha256": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let grant = response_json(response).await;
    assert!(
        grant["upload"]["url"]
            .as_str()
            .unwrap()
            .contains("s3.amazonaws.com")
    );
    let request = json!({
        "from_address": "contact@ahara.io",
        "body_text": "Attached are the notes.",
        "attachments": [{
            "upload_id": grant["upload_id"]
        }]
    });
    assert!(request.to_string().len() < 1024);
    let response = authenticated_request_with_state(
        state.clone(),
        "POST",
        "/mailbox/messages/00000000-0000-0000-0000-000000000001/reply",
        Some(request),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let queued = response_json(response).await;
    let path = format!(
        "/outbound/messages/{}",
        queued["message_id"].as_str().unwrap()
    );
    let response = authenticated_request_with_state(state, "GET", &path, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let detail = response_json(response).await;
    assert_eq!(detail["attachments"][0]["filename"], "notes.txt");
    assert_eq!(detail["attachments"][0]["size_bytes"], 15 * 1024 * 1024);
    assert!(detail["attachments"][0].get("content_base64").is_none());
}

#[tokio::test]
async fn reply_rejects_inline_attachment_bytes() {
    let response = authenticated_request_with_state(ApiState::for_tests(), "POST", "/mailbox/messages/00000000-0000-0000-0000-000000000001/reply", Some(json!({
        "from_address": "contact@ahara.io", "body_text": "body", "attachments": [{ "filename": "x", "content_type": "text/plain", "content_base64": "eA==" }]
    }))).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use burst_server::ws::ServerEvent;
use tower::ServiceExt;

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn setup(pool: &sqlx::PgPool) -> (common::TestApp, String) {
    let app = common::TestApp::new(pool.clone());
    let alice = common::seed_user(pool, "alice").await;
    let ch = common::seed_channel(pool, "general", alice.id).await;
    let ch_id = format!("ch_{}", ch.id);
    (app, ch_id)
}

fn multipart_body(
    boundary: &str,
    content: &str,
    file_name: &str,
    file_content_type: &str,
    file_data: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    // Content field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"content\"\r\n\r\n");
    body.extend_from_slice(content.as_bytes());
    body.extend_from_slice(b"\r\n");
    // File field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"files\"; filename=\"{file_name}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {file_content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(file_data);
    body.extend_from_slice(b"\r\n");
    // End
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

fn multipart_body_multi(
    boundary: &str,
    content: &str,
    files: &[(&str, &str, &[u8])], // (name, content_type, data)
) -> Vec<u8> {
    let mut body = Vec::new();
    // Content field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"content\"\r\n\r\n");
    body.extend_from_slice(content.as_bytes());
    body.extend_from_slice(b"\r\n");
    // File fields
    for (file_name, file_content_type, file_data) in files {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"files\"; filename=\"{file_name}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {file_content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(file_data);
        body.extend_from_slice(b"\r\n");
    }
    // End
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Minimal valid 1x1 RGB PNG for image metadata tests.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8bit RGB
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
    0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
    0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

// ── Tests ─────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn upload_and_download_attachment(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----TestBoundary123";

    let body = multipart_body(
        boundary,
        "check this file",
        "hello.txt",
        "text/plain",
        b"Hello, world!",
    );

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let msg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(msg["content"], "check this file");
    let attachments = msg["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);

    let att = &attachments[0];
    assert_eq!(att["fileName"], "hello.txt");
    assert_eq!(att["contentType"], "text/plain");
    assert_eq!(att["fileSize"], 13); // "Hello, world!" is 13 bytes

    // Download the attachment.
    let att_id = att["id"].as_str().unwrap();
    let (status, download_body) = download(&app, att_id, "alice@test.example").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(download_body, b"Hello, world!");
}

#[sqlx::test(migrations = "../../migrations")]
async fn blocked_extension_rejected(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----TestBoundary456";

    let body = multipart_body(
        boundary,
        "evil",
        "hack.exe",
        "application/octet-stream",
        b"\x00",
    );

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_download(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    // Create a second user who is NOT a member.
    common::seed_user(&pool, "bob").await;

    let boundary = "----TestBoundary789";
    let body = multipart_body(boundary, "secret", "doc.txt", "text/plain", b"classified");

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let msg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let att_id = msg["attachments"][0]["id"].as_str().unwrap();

    // Bob tries to download — should get 404.
    let (status, _) = download(&app, att_id, "bob@test.example").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn file_size_limit_enforced(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----TestBoundarySize";

    // The default max_file_size is 20MB. Create something slightly over.
    let big_data = vec![0u8; 20 * 1024 * 1024 + 1];
    let body = multipart_body(
        boundary,
        "big file",
        "large.bin",
        "application/octet-stream",
        &big_data,
    );

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::from_u16(413).unwrap());
}

#[sqlx::test(migrations = "../../migrations")]
async fn attachments_included_in_list_messages(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----TestBoundaryList";

    let body = multipart_body(
        boundary,
        "with file",
        "notes.txt",
        "text/plain",
        b"some notes",
    );

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List messages and verify attachments are present.
    let (status, body) = app
        .get(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    let atts = items[0]["attachments"].as_array().unwrap();
    assert_eq!(atts.len(), 1);
    assert_eq!(atts[0]["fileName"], "notes.txt");
}

// ── Download helpers ─────────────────────────────────────────────────────────

async fn download(
    app: &common::TestApp,
    attachment_id: &str,
    external_id: &str,
) -> (StatusCode, Vec<u8>) {
    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("GET")
        .uri(format!("/api/attachments/{attachment_id}"))
        .header("x-auth-consumer", external_id)
        .body(Body::empty())
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, bytes.to_vec())
}

/// Download variant that also returns the Content-Disposition header value.
async fn download_with_headers(
    app: &common::TestApp,
    attachment_id: &str,
    external_id: &str,
) -> (StatusCode, Vec<u8>, Option<String>) {
    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("GET")
        .uri(format!("/api/attachments/{attachment_id}"))
        .header("x-auth-consumer", external_id)
        .body(Body::empty())
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let disposition = resp
        .headers()
        .get("content-disposition")
        .map(|v| v.to_str().unwrap().to_owned());
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, bytes.to_vec(), disposition)
}

/// Upload a file via multipart and return the parsed message JSON.
async fn upload_file(
    app: &common::TestApp,
    ch_id: &str,
    external_id: &str,
    content: &str,
    file_name: &str,
    file_content_type: &str,
    file_data: &[u8],
) -> serde_json::Value {
    let boundary = "----UploadHelper";
    let body = multipart_body(boundary, content, file_name, file_content_type, file_data);

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", external_id)
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ── Additional attachment tests ─────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn multiple_files_upload(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----MultiFile";

    let files: Vec<(&str, &str, &[u8])> = vec![
        ("file1.txt", "text/plain", b"first file"),
        ("file2.txt", "text/plain", b"second file"),
    ];
    let body = multipart_body_multi(boundary, "two files", &files);

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let msg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let attachments = msg["attachments"].as_array().unwrap();
    assert_eq!(
        attachments.len(),
        2,
        "both uploaded files must appear in the response"
    );
    assert_eq!(attachments[0]["fileName"], "file1.txt");
    assert_eq!(attachments[1]["fileName"], "file2.txt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn file_only_message_with_empty_content(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let boundary = "----EmptyContent";

    let body = multipart_body(boundary, "", "data.txt", "text/plain", b"some data");

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "file-only message with empty content must succeed"
    );

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let msg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let content = msg["content"].as_str().unwrap();
    assert!(
        !content.is_empty(),
        "content must be a non-empty placeholder for file-only messages"
    );
    assert_eq!(
        msg["attachments"].as_array().unwrap().len(),
        1,
        "attachment must be present"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_member_cannot_upload(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    // Create bob who is NOT a member of the channel.
    common::seed_user(&pool, "bob").await;

    let boundary = "----NonMemberUpload";
    let body = multipart_body(boundary, "sneaky", "hack.txt", "text/plain", b"data");

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "bob@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "non-member must not be able to upload via multipart"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn attachment_id_prefixed_with_att(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let msg = upload_file(
        &app,
        &ch_id,
        "alice@test.example",
        "check prefix",
        "doc.txt",
        "text/plain",
        b"content",
    )
    .await;

    let att_id = msg["attachments"][0]["id"].as_str().unwrap();
    assert!(
        att_id.starts_with("att_"),
        "attachment ID must be prefixed with att_, got: {att_id}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn download_nonexistent_attachment_returns_404(pool: sqlx::PgPool) {
    let (app, _) = setup(&pool).await;

    let fake_id = "att_00000000-0000-0000-0000-000000000000";
    let (status, _) = download(&app, fake_id, "alice@test.example").await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "downloading a nonexistent attachment must return 404"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn content_disposition_inline_for_images(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let msg = upload_file(
        &app,
        &ch_id,
        "alice@test.example",
        "image upload",
        "photo.png",
        "image/png",
        TINY_PNG,
    )
    .await;

    let att_id = msg["attachments"][0]["id"].as_str().unwrap();
    let (status, _, disposition) = download_with_headers(&app, att_id, "alice@test.example").await;

    assert_eq!(status, StatusCode::OK);
    let disp = disposition.expect("Content-Disposition header must be present");
    assert!(
        disp.starts_with("inline"),
        "image downloads must have Content-Disposition: inline, got: {disp}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn content_disposition_attachment_for_other_types(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let msg = upload_file(
        &app,
        &ch_id,
        "alice@test.example",
        "text upload",
        "notes.txt",
        "text/plain",
        b"some notes",
    )
    .await;

    let att_id = msg["attachments"][0]["id"].as_str().unwrap();
    let (status, _, disposition) = download_with_headers(&app, att_id, "alice@test.example").await;

    assert_eq!(status, StatusCode::OK);
    let disp = disposition.expect("Content-Disposition header must be present");
    assert!(
        disp.starts_with("attachment"),
        "non-image downloads must have Content-Disposition: attachment, got: {disp}"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn attachments_included_in_get_message(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let msg = upload_file(
        &app,
        &ch_id,
        "alice@test.example",
        "with attachment",
        "readme.txt",
        "text/plain",
        b"read me",
    )
    .await;

    let msg_id = msg["id"].as_str().unwrap();

    // GET single message
    let (status, body) = app
        .get(
            &format!("/api/channels/{ch_id}/messages/{msg_id}"),
            "alice@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let atts = body["attachments"].as_array().unwrap();
    assert_eq!(atts.len(), 1, "GET single message must include attachments");
    assert_eq!(atts[0]["fileName"], "readme.txt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn attachments_included_in_thread_replies(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    // Post a root message.
    let (_, root) = app
        .post(
            &format!("/api/channels/{ch_id}/messages"),
            "alice@test.example",
            serde_json::json!({ "content": "root message" }),
        )
        .await;
    let root_id = root["id"].as_str().unwrap();

    // Post a thread reply with an attachment.
    let boundary = "----ThreadReply";
    let mut body = Vec::new();
    // content field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"content\"\r\n\r\n");
    body.extend_from_slice(b"reply with file");
    body.extend_from_slice(b"\r\n");
    // threadId field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"threadId\"\r\n\r\n");
    body.extend_from_slice(root_id.as_bytes());
    body.extend_from_slice(b"\r\n");
    // file field
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"files\"; filename=\"reply.txt\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: text/plain\r\n\r\n");
    body.extend_from_slice(b"reply attachment");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List thread replies and verify the attachment is present.
    let (status, replies) = app
        .get(
            &format!("/api/channels/{ch_id}/messages/{root_id}/replies"),
            "alice@test.example",
        )
        .await;

    assert_eq!(status, StatusCode::OK);
    let items = replies["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "there should be one thread reply");
    let atts = items[0]["attachments"].as_array().unwrap();
    assert_eq!(atts.len(), 1, "thread reply must include its attachment");
    assert_eq!(atts[0]["fileName"], "reply.txt");
}

#[sqlx::test(migrations = "../../migrations")]
async fn send_message_with_attachment_broadcasts_event(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;
    let mut rx = app.state.broker.subscribe();

    let boundary = "----BrokerEvent";
    let body = multipart_body(
        boundary,
        "broadcast test",
        "event.txt",
        "text/plain",
        b"event data",
    );

    let req = common::with_peer(Request::builder(), "127.0.0.1:54321")
        .method("POST")
        .uri(format!("/api/channels/{ch_id}/messages"))
        .header("x-auth-consumer", "alice@test.example")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let ServerEvent::MessageCreated { message, .. } = &ev
            && !message.attachments.is_empty()
        {
            assert_eq!(
                message.attachments.len(),
                1,
                "MessageCreated event must include exactly one attachment"
            );
            assert_eq!(message.attachments[0].file_name, "event.txt");
            found = true;
        }
    }
    assert!(
        found,
        "MessageCreated event with attachments must be broadcast"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn image_metadata_extracted(pool: sqlx::PgPool) {
    let (app, ch_id) = setup(&pool).await;

    let msg = upload_file(
        &app,
        &ch_id,
        "alice@test.example",
        "tiny image",
        "pixel.png",
        "image/png",
        TINY_PNG,
    )
    .await;

    let metadata = &msg["attachments"][0]["metadata"];
    assert_eq!(
        metadata["width"], 1,
        "image metadata must report width=1 for the 1x1 PNG"
    );
    assert_eq!(
        metadata["height"], 1,
        "image metadata must report height=1 for the 1x1 PNG"
    );
}

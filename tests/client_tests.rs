use futures::StreamExt;
use ollama_scope::client::stream_chat;
use ollama_scope::stream::StreamEvent;
use ollama_scope::types::{ChatMessage, OllamaChatRequest};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_request() -> OllamaChatRequest {
    OllamaChatRequest {
        model: "llama3".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        stream: true,
    }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("fixtures/{name}")).unwrap()
}

#[tokio::test]
async fn test_normal_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("normal_response.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    let chunks: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Chunk { .. }))
        .collect();
    assert_eq!(chunks.len(), 3, "Should have 3 content chunks");

    let completed = events
        .iter()
        .find(|e| matches!(e, StreamEvent::Completed { .. }));
    assert!(completed.is_some(), "Should have a Completed event");

    // Verify session_id propagation
    for event in &events {
        assert_eq!(event.session_id(), "s1");
    }
}

#[tokio::test]
async fn test_malformed_json() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("malformed.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    let parse_errors: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::ParseError { .. }))
        .collect();
    assert!(
        !parse_errors.is_empty(),
        "Should have at least one ParseError"
    );
}

#[tokio::test]
async fn test_heartbeat_lines() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("heartbeats.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    // Empty lines should be filtered out
    let chunks: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Chunk { .. }))
        .collect();
    assert_eq!(chunks.len(), 2, "Should have 2 real chunks, heartbeats filtered");
}

#[tokio::test]
async fn test_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    let dropped = events.iter().any(|e| matches!(e, StreamEvent::ConnectionDropped { error, .. } if error.contains("500")));
    assert!(dropped, "Should report HTTP 500 as ConnectionDropped");
}

#[tokio::test]
async fn test_http_404() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    let dropped = events.iter().any(|e| matches!(e, StreamEvent::ConnectionDropped { error, .. } if error.contains("404")));
    assert!(dropped, "Should report HTTP 404 as ConnectionDropped");
}

#[tokio::test]
async fn test_session_id_on_all_events() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("normal_response.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "test-session-42")
            .collect()
            .await;

    for event in &events {
        assert_eq!(event.session_id(), "test-session-42");
    }
}

#[tokio::test]
async fn test_single_token_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("single_token.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    let chunks: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, StreamEvent::Chunk { .. }))
        .collect();
    assert_eq!(chunks.len(), 1, "Single token response should have 1 chunk");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, StreamEvent::Completed { .. })),
        "Should complete"
    );
}

#[tokio::test]
async fn test_completed_event_has_metrics() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("normal_response.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    if let Some(StreamEvent::Completed { metrics, .. }) =
        events.iter().find(|e| matches!(e, StreamEvent::Completed { .. }))
    {
        assert_eq!(metrics.total_duration, 5_000_000_000);
        assert_eq!(metrics.eval_count, 3);
        assert_eq!(metrics.prompt_eval_count, 10);
    } else {
        panic!("No Completed event found");
    }
}

#[tokio::test]
async fn test_no_message_field() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("no_message.ndjson")))
        .mount(&server)
        .await;

    let events: Vec<StreamEvent> =
        stream_chat(&server.uri(), &sample_request(), "s1").collect().await;

    // Should handle missing message field gracefully
    let has_completed = events
        .iter()
        .any(|e| matches!(e, StreamEvent::Completed { .. }));
    assert!(has_completed, "Should still complete even without message fields");
}

// axum-based test for connection drop
#[tokio::test]
async fn test_connection_drop() {
    use axum::routing::post;
    use axum::Router;
    use tokio::net::TcpListener;

    let app = Router::new().route(
        "/api/chat",
        post(|| async {
            // Send partial response then drop connection
            let body =
                "{\"model\":\"llama3\",\"created_at\":\"2024-01-01T00:00:00Z\",\"message\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"done\":false}\n";
            axum::response::Response::builder()
                .status(200)
                .body(axum::body::Body::from(body.to_string()))
                .unwrap()
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let events: Vec<StreamEvent> = stream_chat(
        &format!("http://{addr}"),
        &sample_request(),
        "s1",
    )
    .collect()
    .await;

    // Should get the chunk and then a connection dropped
    let has_chunk = events.iter().any(|e| matches!(e, StreamEvent::Chunk { .. }));
    let has_dropped = events
        .iter()
        .any(|e| matches!(e, StreamEvent::ConnectionDropped { .. }));
    assert!(has_chunk, "Should have received at least one chunk");
    assert!(has_dropped, "Should detect connection drop");
}

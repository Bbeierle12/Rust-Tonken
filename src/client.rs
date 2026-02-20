use futures::stream::{self, Stream, StreamExt};
use reqwest::Client;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

use crate::stream::{FinalStreamMetrics, StreamEvent};
use crate::types::OllamaChatRequest;

const CHUNK_TIMEOUT: Duration = Duration::from_secs(60);
const OVERALL_DEADLINE: Duration = Duration::from_secs(300);

/// Stream chat responses from an Ollama API endpoint.
///
/// Returns a stream of `StreamEvent` items. The stream will:
/// - Skip empty lines (heartbeats)
/// - Classify serde errors as `ParseError`
/// - Emit `Completed` when `done: true` with final metrics
/// - Emit `Timeout` on per-chunk or overall deadline timeouts
/// - Emit `ConnectionDropped` on network errors
pub fn stream_chat(
    base_url: &str,
    request: &OllamaChatRequest,
    session_id: &str,
) -> impl Stream<Item = StreamEvent> + Send + 'static {
    let url = format!("{}/api/chat", base_url);
    let session_id = session_id.to_string();
    let request = request.clone();

    stream::once(async move {
        let client = Client::new();
        let result = match tokio::time::timeout(
            CHUNK_TIMEOUT,
            client.post(&url).json(&request).send(),
        )
        .await
        {
            Ok(Ok(resp)) => {
                if !resp.status().is_success() {
                    return stream::iter(vec![StreamEvent::ConnectionDropped {
                        session_id,
                        error: format!("HTTP {}", resp.status()),
                    }])
                    .boxed();
                }
                Ok(resp)
            }
            Ok(Err(e)) => Err(StreamEvent::ConnectionDropped {
                session_id: session_id.clone(),
                error: e.to_string(),
            }),
            Err(_) => Err(StreamEvent::Timeout {
                session_id: session_id.clone(),
            }),
        };

        match result {
            Err(event) => stream::iter(vec![event]).boxed(),
            Ok(resp) => {
                let byte_stream = resp.bytes_stream().map(|result| {
                    result.map_err(std::io::Error::other)
                });
                let reader = StreamReader::new(byte_stream);
                let buf_reader = tokio::io::BufReader::new(reader);
                let lines = buf_reader.lines();

                let session_id_inner = session_id.clone();
                let deadline = tokio::time::Instant::now() + OVERALL_DEADLINE;

                let line_stream = stream::unfold(
                    (lines, session_id_inner, deadline, false),
                    |(mut lines, sid, deadline, done)| async move {
                        if done {
                            return None;
                        }

                        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                        if remaining.is_zero() {
                            return Some((
                                StreamEvent::Timeout {
                                    session_id: sid.clone(),
                                },
                                (lines, sid, deadline, true),
                            ));
                        }

                        let timeout = remaining.min(CHUNK_TIMEOUT);
                        match tokio::time::timeout(timeout, lines.next_line()).await {
                            Ok(Ok(Some(line))) => {
                                let line = line.trim().to_string();
                                if line.is_empty() {
                                    // Skip heartbeat/empty lines
                                    return Some((
                                        StreamEvent::Chunk {
                                            session_id: sid.clone(),
                                            chunk: crate::types::OllamaChatChunk {
                                                model: String::new(),
                                                created_at: String::new(),
                                                message: None,
                                                done: false,
                                                total_duration: None,
                                                load_duration: None,
                                                prompt_eval_count: None,
                                                prompt_eval_duration: None,
                                                eval_count: None,
                                                eval_duration: None,
                                            },
                                        },
                                        (lines, sid, deadline, false),
                                    ));
                                }

                                match serde_json::from_str::<crate::types::OllamaChatChunk>(&line) {
                                    Ok(chunk) if chunk.done => {
                                        let metrics = FinalStreamMetrics {
                                            total_duration: chunk.total_duration.unwrap_or(0),
                                            load_duration: chunk.load_duration.unwrap_or(0),
                                            prompt_eval_count: chunk.prompt_eval_count.unwrap_or(0),
                                            prompt_eval_duration: chunk
                                                .prompt_eval_duration
                                                .unwrap_or(0),
                                            eval_count: chunk.eval_count.unwrap_or(0),
                                            eval_duration: chunk.eval_duration.unwrap_or(0),
                                        };
                                        Some((
                                            StreamEvent::Completed {
                                                session_id: sid.clone(),
                                                metrics,
                                            },
                                            (lines, sid, deadline, true),
                                        ))
                                    }
                                    Ok(chunk) => Some((
                                        StreamEvent::Chunk {
                                            session_id: sid.clone(),
                                            chunk,
                                        },
                                        (lines, sid, deadline, false),
                                    )),
                                    Err(e) => Some((
                                        StreamEvent::ParseError {
                                            session_id: sid.clone(),
                                            error: e.to_string(),
                                        },
                                        (lines, sid, deadline, false),
                                    )),
                                }
                            }
                            Ok(Ok(None)) => {
                                // Stream ended without done=true
                                Some((
                                    StreamEvent::ConnectionDropped {
                                        session_id: sid.clone(),
                                        error: "Stream ended unexpectedly".to_string(),
                                    },
                                    (lines, sid, deadline, true),
                                ))
                            }
                            Ok(Err(e)) => Some((
                                StreamEvent::ConnectionDropped {
                                    session_id: sid.clone(),
                                    error: e.to_string(),
                                },
                                (lines, sid, deadline, true),
                            )),
                            Err(_) => Some((
                                StreamEvent::Timeout {
                                    session_id: sid.clone(),
                                },
                                (lines, sid, deadline, true),
                            )),
                        }
                    },
                );

                // Filter out empty heartbeat chunks
                line_stream
                    .filter(|event| {
                        let keep = match event {
                            StreamEvent::Chunk { chunk, .. } => {
                                // Filter out empty heartbeat chunks (empty model means it was a heartbeat)
                                !chunk.model.is_empty()
                            }
                            _ => true,
                        };
                        futures::future::ready(keep)
                    })
                    .boxed()
            }
        }
    })
    .flatten()
}

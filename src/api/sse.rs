use async_stream::stream;
use futures::{Stream, StreamExt};
use reqwest::Response;

/// A parsed SSE event, mirroring the shape of eventsource_client::SSE
pub enum Sse {
    /// A data event with the raw payload string (the `data:` line value)
    Event(String),
    /// A comment line (`: ...`)
    Comment(String),
    /// The terminal `[DONE]` sentinel — stream should stop after this
    Done,
}

/// Parse a streaming SSE response into a stream of [`Sse`] variants.
///
/// Handles `\n`, `\r`, and `\r\n` line endings correctly, including pairs
/// split across chunk boundaries (tracked via `last_char_was_cr`).
///
/// Yields `Event`, `Comment`, or `Done`. Callers should stop after `Done`.
pub fn sse_stream(response: Response) -> impl Stream<Item = anyhow::Result<Sse>> {
    stream! {
        let mut byte_stream = response.bytes_stream();
        let mut current_line: Vec<u8> = Vec::new();
        let mut pending_data: Option<String> = None;
        let mut last_char_was_cr = false;

        while let Some(chunk) = byte_stream.next().await {
            let bytes = chunk?;

            for &b in bytes.iter() {
                match b {
                    b'\r' => {
                        // Treat \r as a line terminator; set flag to skip a following \n
                        let line = std::mem::take(&mut current_line);
                        last_char_was_cr = true;
                        if let Some(evt) = process_line(&line, &mut pending_data)? {
                            let is_done = matches!(evt, Sse::Done);
                            yield Ok(evt);
                            if is_done {
                                return;
                            }
                        }
                    }
                    b'\n' if last_char_was_cr => {
                        // Second half of \r\n pair — skip, line was already dispatched
                        last_char_was_cr = false;
                    }
                    b'\n' => {
                        last_char_was_cr = false;
                        let line = std::mem::take(&mut current_line);
                        if let Some(evt) = process_line(&line, &mut pending_data)? {
                            let is_done = matches!(evt, Sse::Done);
                            yield Ok(evt);
                            if is_done {
                                return;
                            }
                        }
                    }
                    _ => {
                        last_char_was_cr = false;
                        current_line.push(b);
                    }
                }
            }
        }
    }
}

/// Process a single complete line and update event state.
///
/// Returns `Some(Sse::Done)` when the `[DONE]` sentinel is seen (caller should stop),
/// `Some(Sse::Event | Sse::Comment)` when an event is ready to emit, or `None` otherwise.
fn process_line(line: &[u8], pending_data: &mut Option<String>) -> anyhow::Result<Option<Sse>> {
    if line.is_empty() {
        // Empty line = event boundary: dispatch accumulated data if any
        if let Some(data) = pending_data.take() {
            return Ok(Some(Sse::Event(data)));
        }
        return Ok(None);
    }

    let line_str = std::str::from_utf8(line)?;

    if let Some(data) = line_str.strip_prefix("data: ") {
        if data.trim() == "[DONE]" {
            return Ok(Some(Sse::Done));
        }
        if !data.is_empty() {
            *pending_data = Some(data.to_string());
        }
    } else if let Some(comment) = line_str.strip_prefix(": ") {
        return Ok(Some(Sse::Comment(comment.to_string())));
    }

    Ok(None)
}

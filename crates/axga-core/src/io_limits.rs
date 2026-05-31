//! Bounded I/O helpers for memory-constrained runtime paths.

use axga_shared::error::{AxgaError, AxgaResult};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::fmt::Display;
use std::io::Read;
use std::path::Path;

pub fn read_text_file_bounded(path: impl AsRef<Path>, limit: u64) -> AxgaResult<String> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();
    if size > limit {
        return Err(AxgaError::FileTooLarge {
            path: path.display().to_string(),
            size,
            limit,
        });
    }

    let mut file = std::fs::File::open(path)?;
    let mut content = String::with_capacity(size as usize);
    file.read_to_string(&mut content)?;
    Ok(content)
}

pub fn read_text_file_bounded_io(path: impl AsRef<Path>, limit: u64) -> std::io::Result<String> {
    read_text_file_bounded(path.as_ref(), limit).map_err(|err| match err {
        AxgaError::Io(io_err) => io_err,
        other => std::io::Error::new(std::io::ErrorKind::InvalidData, other),
    })
}

pub async fn response_text_bounded(
    response: reqwest::Response,
    max_bytes: usize,
) -> AxgaResult<String> {
    if let Some(size) = response.content_length() {
        if size > max_bytes as u64 {
            return Err(AxgaError::HttpResponseTooLarge {
                size,
                limit: max_bytes as u64,
            });
        }
    }

    let bytes = collect_limited_bytes(response.bytes_stream(), max_bytes).await?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub async fn collect_limited_bytes<S, E>(mut stream: S, max_bytes: usize) -> AxgaResult<Vec<u8>>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let mut body = Vec::with_capacity(max_bytes.min(8192));
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| AxgaError::Network(err.to_string()))?;
        let next_len = body.len().saturating_add(chunk.len());
        if next_len > max_bytes {
            return Err(AxgaError::HttpResponseTooLarge {
                size: next_len as u64,
                limit: max_bytes as u64,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn bounded_file_read_rejects_oversized_file() {
        let path = std::env::temp_dir().join(format!(
            "axga-oversized-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = std::fs::File::create(&path).unwrap();
        file.set_len(16).unwrap();
        writeln!(file, "x").unwrap();

        let result = read_text_file_bounded(&path, 8);
        let _ = std::fs::remove_file(&path);

        assert!(matches!(result, Err(AxgaError::FileTooLarge { .. })));
    }

    #[tokio::test]
    async fn limited_byte_collection_rejects_oversized_stream() {
        let chunks = stream::iter([
            Ok::<_, std::io::Error>(Bytes::from_static(b"12345")),
            Ok::<_, std::io::Error>(Bytes::from_static(b"67890")),
        ]);

        let result = collect_limited_bytes(chunks, 8).await;

        assert!(matches!(
            result,
            Err(AxgaError::HttpResponseTooLarge { .. })
        ));
    }
}

use bytes::BytesMut;
use tokio::io::AsyncReadExt;

use harrow_codec_h1::{CodecError, MAX_HEADER_BUF, ParsedRequest, try_parse_request};

use crate::ServerConfig;
use crate::h1::error::write_error;

pub(crate) async fn read_request_head<S>(
    stream: &mut S,
    buf: &mut BytesMut,
    config: &ServerConfig,
) -> Option<ParsedRequest>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let request_started = std::time::Instant::now();

    loop {
        match try_parse_request(buf) {
            Ok(parsed) => return Some(parsed),
            Err(CodecError::Incomplete) => {
                if buf.len() >= MAX_HEADER_BUF {
                    write_error(stream, 400, "request headers too large").await;
                    return None;
                }
            }
            Err(CodecError::Invalid(_)) => {
                write_error(stream, 400, "bad request").await;
                return None;
            }
            Err(CodecError::BodyTooLarge) => {
                write_error(stream, 413, "payload too large").await;
                return None;
            }
        }

        if let Some(timeout) = config.header_read_timeout {
            let remaining = timeout.saturating_sub(request_started.elapsed());
            if remaining.is_zero() {
                return None;
            }
            match tokio::time::timeout(remaining, stream.read_buf(buf)).await {
                Ok(Ok(0)) => return None,
                Ok(Ok(_)) => {}
                Ok(Err(_)) => return None,
                Err(_) => return None,
            }
        } else {
            match stream.read_buf(buf).await {
                Ok(0) => return None,
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }
}

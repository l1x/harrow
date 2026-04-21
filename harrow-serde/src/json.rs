use std::cell::RefCell;
use std::collections::HashMap;

use bytes::Bytes;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use serde_json::Error;

pub const CONTENT_TYPE: &str = "application/json";

const DEFAULT_JSON_CAPACITY: usize = 256;
const MAX_CACHED_JSON_CAPACITY: usize = 256 * 1024;
const MAX_POOLED_JSON_BUFFERS: usize = 8;

thread_local! {
    static JSON_CAPACITY_HINTS: RefCell<HashMap<&'static str, usize>> =
        RefCell::new(HashMap::new());
    static JSON_SCRATCH_BUFFERS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

struct JsonScratchBuffer {
    buf: Vec<u8>,
}

impl AsRef<[u8]> for JsonScratchBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.buf
    }
}

impl Drop for JsonScratchBuffer {
    fn drop(&mut self) {
        recycle_json_buffer(std::mem::take(&mut self.buf));
    }
}

/// Serialize a value to JSON as `Bytes`, writing directly into a reusable thread-local buffer.
pub fn serialize<T: Serialize>(value: &T) -> Result<Bytes, Error> {
    let type_name = std::any::type_name::<T>();
    let capacity = json_capacity_hint(type_name);
    let mut buf = acquire_json_buffer(capacity);

    if let Err(err) = serde_json::to_writer(&mut buf, value) {
        recycle_json_buffer(buf);
        return Err(err);
    }

    let len = buf.len();
    update_json_capacity_hint(type_name, len);
    Ok(Bytes::from_owner(JsonScratchBuffer { buf }))
}

fn json_capacity_hint(type_name: &'static str) -> usize {
    JSON_CAPACITY_HINTS.with(|hints| {
        hints
            .borrow()
            .get(type_name)
            .copied()
            .unwrap_or(DEFAULT_JSON_CAPACITY)
    })
}

fn update_json_capacity_hint(type_name: &'static str, len: usize) {
    let next_hint = next_json_capacity_hint(len);
    JSON_CAPACITY_HINTS.with(|hints| {
        let mut hints = hints.borrow_mut();
        hints
            .entry(type_name)
            .and_modify(|hint| *hint = (*hint).max(next_hint))
            .or_insert(next_hint);
    });
}

fn next_json_capacity_hint(len: usize) -> usize {
    len.max(DEFAULT_JSON_CAPACITY)
        .next_power_of_two()
        .min(MAX_CACHED_JSON_CAPACITY)
}

fn acquire_json_buffer(capacity: usize) -> Vec<u8> {
    let target_capacity = next_json_capacity_hint(capacity);
    JSON_SCRATCH_BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        let mut buf = buffers.pop().unwrap_or_default();
        buf.clear();
        if buf.capacity() < target_capacity {
            buf.reserve(target_capacity - buf.capacity());
        }
        buf
    })
}

fn recycle_json_buffer(mut buf: Vec<u8>) {
    if buf.capacity() > MAX_CACHED_JSON_CAPACITY {
        return;
    }

    buf.clear();
    JSON_SCRATCH_BUFFERS.with(|buffers| {
        let mut buffers = buffers.borrow_mut();
        if buffers.len() < MAX_POOLED_JSON_BUFFERS {
            buffers.push(buf);
        }
    });
}

/// Deserialize a value from a JSON byte slice.
pub fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Sample {
        name: String,
        value: u32,
    }

    #[test]
    fn round_trip() {
        let input = Sample {
            name: "test".into(),
            value: 42,
        };
        let bytes = serialize(&input).unwrap();
        let output: Sample = deserialize(&bytes).unwrap();
        assert_eq!(input, output);
    }

    #[test]
    fn serialize_produces_valid_json() {
        let input = Sample {
            name: "hello".into(),
            value: 1,
        };
        let bytes = serialize(&input).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["name"], "hello");
        assert_eq!(parsed["value"], 1);
    }

    #[test]
    fn deserialize_error_on_invalid_input() {
        let result = deserialize::<Sample>(b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn content_type_is_correct() {
        assert_eq!(CONTENT_TYPE, "application/json");
    }

    #[derive(Serialize)]
    struct LargeSample {
        users: Vec<Sample>,
    }

    fn drain_json_buffer_pool() -> usize {
        JSON_SCRATCH_BUFFERS.with(|buffers| {
            let mut buffers = buffers.borrow_mut();
            let len = buffers.len();
            buffers.clear();
            len
        })
    }

    #[test]
    fn capacity_hint_grows_for_repeated_large_type() {
        let input = LargeSample {
            users: (0..128)
                .map(|i| Sample {
                    name: format!("user-{i}"),
                    value: i,
                })
                .collect(),
        };

        let type_name = std::any::type_name::<LargeSample>();
        assert_eq!(json_capacity_hint(type_name), DEFAULT_JSON_CAPACITY);

        let bytes = serialize(&input).unwrap();
        let hint = json_capacity_hint(type_name);

        assert!(hint >= bytes.len());
        assert!(hint > DEFAULT_JSON_CAPACITY);

        let bytes_again = serialize(&input).unwrap();
        assert_eq!(bytes, bytes_again);
        assert_eq!(json_capacity_hint(type_name), hint);
    }

    #[test]
    fn serialize_returns_buffer_to_thread_local_pool_on_drop() {
        drain_json_buffer_pool();

        let input = LargeSample {
            users: (0..32)
                .map(|i| Sample {
                    name: format!("user-{i}"),
                    value: i,
                })
                .collect(),
        };

        let bytes = serialize(&input).unwrap();
        assert_eq!(drain_json_buffer_pool(), 0);

        drop(bytes);
        assert_eq!(drain_json_buffer_pool(), 1);
    }
}

//! Safe I/O operation types.
//!
//! Each operation owns its buffers and is cancellation-safe: dropping the
//! future before completion never causes use-after-free.

pub mod accept;
pub mod cancel;
pub mod connect;
pub mod read;
pub mod recv;
pub mod send;
pub mod timeout;
pub mod write;

pub use accept::Accept;
pub use cancel::Cancel;
pub use connect::Connect;
pub use read::Read;
pub use recv::Recv;
pub use send::Send;
pub use timeout::Timeout;
pub use write::Write;

//! Provides transport over Unix Domain Sockets.

mod stream;
pub use stream::{connect, Connection, Stream};
#[cfg(feature = "server")]
mod listener;
#[cfg(feature = "server")]
pub use listener::{bind, Listener};

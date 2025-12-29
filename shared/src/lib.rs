//! Shared types between daemon and GUI

pub mod group;
pub mod protocol;
pub mod session;

pub use group::Group;
pub use protocol::*;
pub use session::{Session, SessionStatus};

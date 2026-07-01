//! Connection state management for IRC client.

use std::fmt;

/// High-level connection state for an IRC client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    /// No socket or socket closed
    #[default]
    Disconnected,
    /// TCP/TLS socket is connecting
    Connecting,
    /// Socket open, performing CAP/NICK/USER handshake
    Negotiating,
    /// IRC registered, ready to send/receive messages
    Ready,
    /// Was connected, now attempting to reconnect
    Reconnecting,
    /// Connection failed with an error
    Error,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "disconnected"),
            ConnectionState::Connecting => write!(f, "connecting"),
            ConnectionState::Negotiating => write!(f, "negotiating"),
            ConnectionState::Ready => write!(f, "ready"),
            ConnectionState::Reconnecting => write!(f, "reconnecting"),
            ConnectionState::Error => write!(f, "error"),
        }
    }
}

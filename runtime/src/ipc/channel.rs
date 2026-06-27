//! Core IPC types: ChannelId, IpcError, Channel trait.

use serde::{Deserialize, Serialize};
use crate::{AgentId, Message};

/// Unique identifier for an IPC channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(uuid::Uuid);

impl ChannelId {
    /// Create a new unique channel ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for ChannelId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// IPC errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IpcError {
    /// Channel buffer is full (backpressure).
    #[error("IPC channel is full")]
    ChannelFull,

    /// Channel does not exist.
    #[error("IPC channel not found")]
    ChannelNotFound,

    /// Agent is not authorized for this operation.
    #[error("unauthorized IPC operation")]
    Unauthorized,

    /// Receiver is not part of this channel.
    #[error("receiver not in channel")]
    InvalidReceiver,

    /// Sender is not the owner of this channel.
    #[error("sender not channel owner")]
    InvalidSender,
}

/// Type of IPC channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType {
    /// Point-to-point (unidirectional, one sender → one receiver).
    Unidirectional,
    /// One-to-many broadcast (one sender → multiple receivers).
    Broadcast,
}

/// Channel trait — common interface for all channel types.
pub trait Channel: std::fmt::Debug + Send + Sync {
    /// Get the channel ID.
    fn id(&self) -> ChannelId;

    /// Get the channel type.
    fn channel_type(&self) -> ChannelType;

    /// Get the sender agent ID.
    fn sender(&self) -> AgentId;

    /// Send a message through the channel.
    ///
    /// Returns `IpcError::ChannelFull` if the buffer is full (backpressure).
    fn send(&mut self, msg: Message) -> Result<(), IpcError>;

    /// Poll for messages destined to a specific receiver.
    ///
    /// Returns all pending messages for the receiver (may be empty).
    fn poll(&mut self, receiver: AgentId) -> Vec<Message>;

    /// Get the number of pending messages for a receiver.
    fn pending_count(&self, receiver: AgentId) -> usize;

    /// Check if the channel buffer is full.
    fn is_full(&self) -> bool;

    /// Get the channel capacity.
    fn capacity(&self) -> usize;
}

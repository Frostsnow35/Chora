//! IpcBroker — IPC channel manager and message router.

use std::collections::HashMap;
use crate::{AgentId, Message};
use super::{Channel, ChannelId, IpcError, UnidirectionalChannel, BroadcastChannel};

/// IPC Broker — manages all channels and routes messages.
///
/// The broker is the central authority for IPC:
/// - Creates and destroys channels
/// - Routes messages from senders to receivers
/// - Provides polling interface for receivers
///
/// # Architecture
///
/// The broker holds all channels in a HashMap, keyed by ChannelId.
/// When an agent sends a message, the broker finds the channel and
/// calls `channel.send()`. When an agent polls, the broker calls
/// `channel.poll()`.
#[derive(Debug)]
pub struct IpcBroker {
    /// All channels, keyed by ChannelId.
    channels: HashMap<ChannelId, Box<dyn Channel>>,
}

impl IpcBroker {
    /// Create a new IPC broker with no channels.
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Create a point-to-point (unidirectional) channel.
    ///
    /// Returns the ChannelId on success.
    pub fn create_p2p_channel(
        &mut self,
        sender: AgentId,
        receiver: AgentId,
        capacity: usize,
    ) -> Result<ChannelId, IpcError> {
        let channel = UnidirectionalChannel::new(sender, receiver, capacity);
        let id = channel.id();
        self.channels.insert(id, Box::new(channel));
        Ok(id)
    }

    /// Create a broadcast channel (one-to-many).
    ///
    /// Returns the ChannelId on success.
    pub fn create_broadcast_channel(
        &mut self,
        sender: AgentId,
        receivers: Vec<AgentId>,
        capacity: usize,
    ) -> Result<ChannelId, IpcError> {
        let channel = BroadcastChannel::new(sender, receivers, capacity);
        let id = channel.id();
        self.channels.insert(id, Box::new(channel));
        Ok(id)
    }

    /// Destroy a channel.
    ///
    /// Returns true if the channel existed and was removed.
    pub fn destroy_channel(&mut self, channel_id: ChannelId) -> bool {
        self.channels.remove(&channel_id).is_some()
    }

    /// Send a message through a channel.
    ///
    /// Returns `IpcError::ChannelNotFound` if the channel doesn't exist.
    /// Returns `IpcError::ChannelFull` if the channel's buffer is full.
    pub fn send(&mut self, channel_id: ChannelId, msg: Message) -> Result<(), IpcError> {
        let channel = self
            .channels
            .get_mut(&channel_id)
            .ok_or(IpcError::ChannelNotFound)?;

        channel.send(msg)
    }

    /// Poll for messages from a specific channel for a receiver.
    ///
    /// Returns all pending messages (may be empty).
    pub fn poll(&mut self, channel_id: ChannelId, receiver: AgentId) -> Vec<Message> {
        match self.channels.get_mut(&channel_id) {
            Some(channel) => channel.poll(receiver),
            None => vec![],
        }
    }

    /// Poll for messages from all channels for a receiver.
    ///
    /// Returns all pending messages from all channels where this agent is a receiver.
    pub fn poll_all(&mut self, receiver: AgentId) -> Vec<Message> {
        let mut all_messages = vec![];
        for channel in self.channels.values_mut() {
            let messages = channel.poll(receiver);
            all_messages.extend(messages);
        }
        all_messages
    }

    /// Get the number of pending messages for a receiver across all channels.
    pub fn pending_count(&self, receiver: AgentId) -> usize {
        self.channels
            .values()
            .map(|channel| channel.pending_count(receiver))
            .sum()
    }

    /// Check if a channel exists.
    pub fn channel_exists(&self, channel_id: ChannelId) -> bool {
        self.channels.contains_key(&channel_id)
    }

    /// Get the number of channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get all channel IDs.
    pub fn channel_ids(&self) -> Vec<ChannelId> {
        self.channels.keys().copied().collect()
    }

    /// Check if any channel is full (for monitoring/backpressure detection).
    pub fn has_full_channels(&self) -> bool {
        self.channels.values().any(|c| c.is_full())
    }
}

impl Default for IpcBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MessageMetadata;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    fn test_message(from: AgentId, to: AgentId, content: &str) -> Message {
        Message {
            sender: from,
            receiver: to,
            message_type: "test".to_string(),
            payload: serde_json::json!({ "content": content }),
            metadata: MessageMetadata {
                sent_at: chrono::Utc::now(),
                priority: 0,
                requires_ack: false,
            },
        }
    }

    #[test]
    fn test_broker_creation() {
        let broker = IpcBroker::new();
        assert_eq!(broker.channel_count(), 0);
    }

    #[test]
    fn test_broker_create_p2p_channel() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        assert!(broker.channel_exists(channel_id));
        assert_eq!(broker.channel_count(), 1);
    }

    #[test]
    fn test_broker_create_broadcast_channel() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receivers = vec![test_agent_id(2), test_agent_id(3)];

        let channel_id = broker.create_broadcast_channel(sender, receivers, 10).unwrap();
        assert!(broker.channel_exists(channel_id));
        assert_eq!(broker.channel_count(), 1);
    }

    #[test]
    fn test_broker_destroy_channel() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        assert!(broker.channel_exists(channel_id));

        assert!(broker.destroy_channel(channel_id));
        assert!(!broker.channel_exists(channel_id));
        assert_eq!(broker.channel_count(), 0);
    }

    #[test]
    fn test_broker_send_and_poll() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        let msg = test_message(sender, receiver, "hello");

        assert!(broker.send(channel_id, msg).is_ok());
        assert_eq!(broker.pending_count(receiver), 1);

        let messages = broker.poll(channel_id, receiver);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].payload["content"], "hello");
    }

    #[test]
    fn test_broker_send_channel_not_found() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let msg = test_message(sender, receiver, "hello");

        let fake_id = ChannelId::new();
        assert!(matches!(broker.send(fake_id, msg), Err(IpcError::ChannelNotFound)));
    }

    #[test]
    fn test_broker_poll_all() {
        let mut broker = IpcBroker::new();
        let sender1 = test_agent_id(1);
        let sender2 = test_agent_id(2);
        let receiver = test_agent_id(3);

        let chan1 = broker.create_p2p_channel(sender1, receiver, 10).unwrap();
        let chan2 = broker.create_p2p_channel(sender2, receiver, 10).unwrap();

        let msg1 = test_message(sender1, receiver, "from sender1");
        let msg2 = test_message(sender2, receiver, "from sender2");

        broker.send(chan1, msg1).unwrap();
        broker.send(chan2, msg2).unwrap();

        assert_eq!(broker.pending_count(receiver), 2);

        let all_messages = broker.poll_all(receiver);
        assert_eq!(all_messages.len(), 2);
    }

    #[test]
    fn test_broker_channel_ids() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let id1 = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        let id2 = broker.create_broadcast_channel(sender, vec![receiver], 10).unwrap();

        let ids = broker.channel_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_broker_has_full_channels() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 2).unwrap();

        assert!(!broker.has_full_channels());

        // Fill the channel
        for i in 0..2 {
            let msg = test_message(sender, receiver, &format!("msg{}", i));
            broker.send(channel_id, msg).unwrap();
        }

        assert!(broker.has_full_channels());
    }
}

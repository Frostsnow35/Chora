//! BroadcastChannel — one-to-many IPC (pub-sub-like).

use std::collections::{HashMap, VecDeque};
use crate::{AgentId, Message};
use super::{Channel, ChannelId, ChannelType, IpcError};

/// One-to-many broadcast channel (pub-sub-like).
///
/// One sender → multiple receivers. Each receiver has an independent queue.
/// When a message is sent, it is copied to all receivers' queues.
#[derive(Debug)]
pub struct BroadcastChannel {
    /// Unique channel identifier.
    id: ChannelId,
    /// Sender agent ID.
    sender: AgentId,
    /// Receiver agent IDs.
    receivers: Vec<AgentId>,
    /// Per-receiver message queues.
    queues: HashMap<AgentId, VecDeque<Message>>,
    /// Maximum capacity per receiver.
    capacity: usize,
}

impl BroadcastChannel {
    /// Create a new broadcast channel.
    ///
    /// # Arguments
    ///
    /// * `sender` - The agent that can send messages.
    /// * `receivers` - The agents that will receive messages.
    /// * `capacity` - Maximum number of messages per receiver queue.
    pub fn new(sender: AgentId, receivers: Vec<AgentId>, capacity: usize) -> Self {
        let mut queues = HashMap::new();
        for &receiver in &receivers {
            queues.insert(receiver, VecDeque::with_capacity(capacity));
        }

        Self {
            id: ChannelId::new(),
            sender,
            receivers,
            queues,
            capacity,
        }
    }

    /// Check if a receiver is part of this channel.
    pub fn has_receiver(&self, receiver: AgentId) -> bool {
        self.receivers.contains(&receiver)
    }

    /// Get all receivers.
    pub fn receivers(&self) -> &[AgentId] {
        &self.receivers
    }
}

impl Channel for BroadcastChannel {
    fn id(&self) -> ChannelId {
        self.id
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Broadcast
    }

    fn sender(&self) -> AgentId {
        self.sender
    }

    fn send(&mut self, msg: Message) -> Result<(), IpcError> {
        // Check if sender is authorized
        if msg.sender != self.sender {
            return Err(IpcError::InvalidSender);
        }

        // Check if all receivers' queues have space
        // (we check before copying to ensure atomicity)
        for &receiver in &self.receivers {
            if let Some(queue) = self.queues.get(&receiver) {
                if queue.len() >= self.capacity {
                    return Err(IpcError::ChannelFull);
                }
            }
        }

        // Copy message to all receivers
        for &receiver in &self.receivers {
            if let Some(queue) = self.queues.get_mut(&receiver) {
                let mut msg_copy = msg.clone();
                msg_copy.receiver = receiver;
                queue.push_back(msg_copy);
            }
        }

        Ok(())
    }

    fn poll(&mut self, receiver: AgentId) -> Vec<Message> {
        // Only designated receivers can poll
        if !self.receivers.contains(&receiver) {
            return vec![];
        }

        if let Some(queue) = self.queues.get_mut(&receiver) {
            queue.drain(..).collect()
        } else {
            vec![]
        }
    }

    fn pending_count(&self, receiver: AgentId) -> usize {
        if !self.receivers.contains(&receiver) {
            return 0;
        }

        self.queues.get(&receiver).map(|q| q.len()).unwrap_or(0)
    }

    fn is_full(&self) -> bool {
        // Channel is full if any receiver's queue is full
        self.queues.values().any(|q| q.len() >= self.capacity)
    }

    fn capacity(&self) -> usize {
        self.capacity
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
    fn test_broadcast_creation() {
        let sender = test_agent_id(1);
        let receivers = vec![test_agent_id(2), test_agent_id(3)];
        let channel = BroadcastChannel::new(sender, receivers.clone(), 10);

        assert_eq!(channel.sender(), sender);
        assert_eq!(channel.receivers(), receivers.as_slice());
        assert_eq!(channel.capacity(), 10);
    }

    #[test]
    fn test_broadcast_send_to_all() {
        let sender = test_agent_id(1);
        let receiver1 = test_agent_id(2);
        let receiver2 = test_agent_id(3);
        let receivers = vec![receiver1, receiver2];
        let mut channel = BroadcastChannel::new(sender, receivers, 10);

        let msg = test_message(sender, receiver1, "broadcast");
        assert!(channel.send(msg).is_ok());

        // Both receivers should have the message
        assert_eq!(channel.pending_count(receiver1), 1);
        assert_eq!(channel.pending_count(receiver2), 1);
    }

    #[test]
    fn test_broadcast_independent_queues() {
        let sender = test_agent_id(1);
        let receiver1 = test_agent_id(2);
        let receiver2 = test_agent_id(3);
        let receivers = vec![receiver1, receiver2];
        let mut channel = BroadcastChannel::new(sender, receivers, 10);

        let msg = test_message(sender, receiver1, "msg");
        channel.send(msg).unwrap();

        // receiver1 polls and drains
        let messages1 = channel.poll(receiver1);
        assert_eq!(messages1.len(), 1);
        assert_eq!(channel.pending_count(receiver1), 0);

        // receiver2 still has the message
        assert_eq!(channel.pending_count(receiver2), 1);
        let messages2 = channel.poll(receiver2);
        assert_eq!(messages2.len(), 1);
    }

    #[test]
    fn test_broadcast_backpressure() {
        let sender = test_agent_id(1);
        let receiver1 = test_agent_id(2);
        let receiver2 = test_agent_id(3);
        let receivers = vec![receiver1, receiver2];
        let mut channel = BroadcastChannel::new(sender, receivers, 2);

        // Fill receiver1's queue
        for i in 0..2 {
            let msg = test_message(sender, receiver1, &format!("msg{}", i));
            channel.send(msg).unwrap();
        }

        // Now try to send one more — should fail (ChannelFull)
        let msg = test_message(sender, receiver1, "msg2");
        assert!(matches!(channel.send(msg), Err(IpcError::ChannelFull)));
    }

    #[test]
    fn test_broadcast_wrong_sender() {
        let sender = test_agent_id(1);
        let wrong_sender = test_agent_id(99);
        let receiver = test_agent_id(2);
        let receivers = vec![receiver];
        let mut channel = BroadcastChannel::new(sender, receivers, 10);

        let msg = test_message(wrong_sender, receiver, "hello");
        assert!(matches!(channel.send(msg), Err(IpcError::InvalidSender)));
    }

    #[test]
    fn test_broadcast_has_receiver() {
        let sender = test_agent_id(1);
        let receiver1 = test_agent_id(2);
        let receiver2 = test_agent_id(3);
        let not_receiver = test_agent_id(99);
        let receivers = vec![receiver1, receiver2];
        let channel = BroadcastChannel::new(sender, receivers, 10);

        assert!(channel.has_receiver(receiver1));
        assert!(channel.has_receiver(receiver2));
        assert!(!channel.has_receiver(not_receiver));
    }
}

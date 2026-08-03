//! UnidirectionalChannel — point-to-point IPC (pipe-like).

use std::collections::VecDeque;
use crate::{AgentId, Message};
use super::{Channel, ChannelId, ChannelType, IpcError};

/// Point-to-point unidirectional channel (pipe-like).
///
/// One sender → one receiver, FIFO order, bounded buffer.
/// When the buffer is full, `send()` returns `IpcError::ChannelFull`.
#[derive(Debug)]
pub struct UnidirectionalChannel {
    /// Unique channel identifier.
    id: ChannelId,
    /// Sender agent ID.
    sender: AgentId,
    /// Receiver agent ID.
    receiver: AgentId,
    /// Message queue (FIFO).
    queue: VecDeque<Message>,
    /// Maximum capacity.
    capacity: usize,
}

impl UnidirectionalChannel {
    /// Create a new unidirectional channel.
    ///
    /// # Arguments
    ///
    /// * `sender` - The agent that can send messages.
    /// * `receiver` - The agent that will receive messages.
    /// * `capacity` - Maximum number of messages in the queue.
    pub fn new(sender: AgentId, receiver: AgentId, capacity: usize) -> Self {
        Self {
            id: ChannelId::new(),
            sender,
            receiver,
            queue: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
}

impl Channel for UnidirectionalChannel {
    fn id(&self) -> ChannelId {
        self.id
    }

    fn channel_type(&self) -> ChannelType {
        ChannelType::Unidirectional
    }

    fn sender(&self) -> AgentId {
        self.sender
    }

    fn send(&mut self, msg: Message) -> Result<(), IpcError> {
        // Check if sender is authorized
        if msg.sender != self.sender {
            return Err(IpcError::InvalidSender);
        }

        // Check if receiver is correct
        if msg.receiver != self.receiver {
            return Err(IpcError::InvalidReceiver);
        }

        // Check capacity (backpressure)
        if self.queue.len() >= self.capacity {
            return Err(IpcError::ChannelFull);
        }

        self.queue.push_back(msg);
        Ok(())
    }

    fn poll(&mut self, receiver: AgentId) -> Vec<Message> {
        // Only the designated receiver can poll
        if receiver != self.receiver {
            return vec![];
        }

        // Drain all messages
        self.queue.drain(..).collect()
    }

    fn pending_count(&self, receiver: AgentId) -> usize {
        if receiver != self.receiver {
            return 0;
        }
        self.queue.len()
    }

    fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
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
    fn test_unidirectional_creation() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let channel = UnidirectionalChannel::new(sender, receiver, 10);

        assert_eq!(channel.sender(), sender);
        assert_eq!(channel.capacity(), 10);
        assert_eq!(channel.pending_count(receiver), 0);
        assert!(!channel.is_full());
    }

    #[test]
    fn test_unidirectional_send_success() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let msg = test_message(sender, receiver, "hello");
        assert!(channel.send(msg).is_ok());
        assert_eq!(channel.pending_count(receiver), 1);
    }

    #[test]
    fn test_unidirectional_send_wrong_sender() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let wrong_sender = test_agent_id(3);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let msg = test_message(wrong_sender, receiver, "hello");
        assert!(matches!(channel.send(msg), Err(IpcError::InvalidSender)));
    }

    #[test]
    fn test_unidirectional_send_wrong_receiver() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let wrong_receiver = test_agent_id(3);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let msg = test_message(sender, wrong_receiver, "hello");
        assert!(matches!(channel.send(msg), Err(IpcError::InvalidReceiver)));
    }

    #[test]
    fn test_unidirectional_poll_empty() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let messages = channel.poll(receiver);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_unidirectional_poll_drains_queue() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        for i in 0..3 {
            let msg = test_message(sender, receiver, &format!("msg{}", i));
            channel.send(msg).unwrap();
        }

        let messages = channel.poll(receiver);
        assert_eq!(messages.len(), 3);
        assert_eq!(channel.pending_count(receiver), 0); // drained
    }

    #[test]
    fn test_unidirectional_poll_wrong_receiver() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let wrong_receiver = test_agent_id(3);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let msg = test_message(sender, receiver, "hello");
        channel.send(msg).unwrap();

        let messages = channel.poll(wrong_receiver);
        assert!(messages.is_empty());
        assert_eq!(channel.pending_count(receiver), 1); // still there
    }
}

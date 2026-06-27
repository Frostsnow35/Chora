//! IPC Subsystem — OS-inspired inter-agent communication.
//!
//! This module implements **Inter-Process Communication (IPC)** for agents,
//! following OS design principles:
//!
//! - **Pipes (UnidirectionalChannel)**: P2P, FIFO, bounded buffer
//! - **Message Queues (BroadcastChannel)**: One-to-many, independent queues
//! - **Backpressure**: Bounded queues, return error when full
//!
//! # Architecture
//!
//! ```text
//! Agent A                    IpcBroker                    Agent B
//!    │                          │                            │
//!    │── SendIpc(chan, msg) ───>│                            │
//!    │                          │── enqueue ──>              │
//!    │                          │── notify scheduler ───────>│
//!    │                          │     (on_ready for B)        │
//!    │                          │                            │
//!    │                          │     (B's step() called)    │
//!    │                          │<── poll(chan) ─────────────│
//!    │                          │── return msgs ────────────>│
//! ```
//!
//! # Design Decisions
//!
//! - **Topology**: P2P (UnidirectionalChannel) + Broadcast (BroadcastChannel)
//! - **Buffering**: Bounded queues, backpressure via `IpcError::ChannelFull`
//! - **Channel Creation**: Dynamic at runtime (via `StepOutput::CreateChannel`)
//! - **Sovereignty**: IPC is User Space (no restrictions, future: audit only)
//!
//! # Integration
//!
//! - `StepOutput::SendIpc` — Agent requests to send a message
//! - `StepOutput::CreateChannel` — Agent requests to create a channel
//! - `StepContext::pending_messages` — Reused for receiving messages
//! - `Runtime` holds `IpcBroker` for routing

mod channel;
mod unidirectional;
mod broadcast;
mod broker;

pub use channel::{ChannelId, IpcError, Channel, ChannelType};
pub use unidirectional::UnidirectionalChannel;
pub use broadcast::BroadcastChannel;
pub use broker::IpcBroker;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, Message, MessageMetadata};
    use std::collections::HashMap;

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

    // ========== ChannelId Tests ==========

    #[test]
    fn test_channel_id_creation() {
        let id1 = ChannelId::new();
        let id2 = ChannelId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_channel_id_clone_eq() {
        let id1 = ChannelId::new();
        let id2 = id1;
        assert_eq!(id1, id2);
    }

    // ========== IpcError Tests ==========

    #[test]
    fn test_ipc_error_display() {
        let err = IpcError::ChannelFull;
        assert!(format!("{}", err).contains("full"));
    }

    // ========== UnidirectionalChannel Tests ==========

    #[test]
    fn test_unidirectional_send_and_poll() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        let msg = test_message(sender, receiver, "hello");
        assert!(channel.send(msg.clone()).is_ok());

        let received = channel.poll(receiver);
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].payload["content"], "hello");
    }

    #[test]
    fn test_unidirectional_bounded_queue() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 2);

        let msg1 = test_message(sender, receiver, "msg1");
        let msg2 = test_message(sender, receiver, "msg2");
        let msg3 = test_message(sender, receiver, "msg3");

        assert!(channel.send(msg1).is_ok());
        assert!(channel.send(msg2).is_ok());
        assert!(matches!(channel.send(msg3), Err(IpcError::ChannelFull)));
    }

    #[test]
    fn test_unidirectional_fifo_order() {
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let mut channel = UnidirectionalChannel::new(sender, receiver, 10);

        for i in 0..5 {
            let msg = test_message(sender, receiver, &format!("msg{}", i));
            channel.send(msg).unwrap();
        }

        let received = channel.poll(receiver);
        assert_eq!(received.len(), 5);
        for (i, msg) in received.iter().enumerate() {
            assert_eq!(msg.payload["content"], format!("msg{}", i));
        }
    }

    // ========== BroadcastChannel Tests ==========

    #[test]
    fn test_broadcast_send_to_all() {
        let sender = test_agent_id(1);
        let receivers = vec![test_agent_id(2), test_agent_id(3), test_agent_id(4)];
        let mut channel = BroadcastChannel::new(sender, receivers.clone(), 10);

        let msg = test_message(sender, receivers[0], "broadcast");
        assert!(channel.send(msg).is_ok());

        // Each receiver should get the message
        for &receiver in &receivers {
            let received = channel.poll(receiver);
            assert_eq!(received.len(), 1);
            assert_eq!(received[0].payload["content"], "broadcast");
        }
    }

    #[test]
    fn test_broadcast_independent_queues() {
        let sender = test_agent_id(1);
        let receiver1 = test_agent_id(2);
        let receiver2 = test_agent_id(3);
        let mut channel = BroadcastChannel::new(sender, vec![receiver1, receiver2], 10);

        let msg = test_message(sender, receiver1, "msg");
        channel.send(msg).unwrap();

        // receiver1 polls and gets the message
        let received1 = channel.poll(receiver1);
        assert_eq!(received1.len(), 1);

        // receiver2 should still have the message
        let received2 = channel.poll(receiver2);
        assert_eq!(received2.len(), 1);
    }

    // ========== IpcBroker Tests ==========

    #[test]
    fn test_broker_create_p2p_channel() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        assert!(broker.channel_exists(channel_id));
    }

    #[test]
    fn test_broker_create_broadcast_channel() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receivers = vec![test_agent_id(2), test_agent_id(3)];

        let channel_id = broker.create_broadcast_channel(sender, receivers, 10).unwrap();
        assert!(broker.channel_exists(channel_id));
    }

    #[test]
    fn test_broker_send_and_poll() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);

        let channel_id = broker.create_p2p_channel(sender, receiver, 10).unwrap();
        let msg = test_message(sender, receiver, "hello");

        assert!(broker.send(channel_id, msg).is_ok());

        let received = broker.poll(channel_id, receiver);
        assert_eq!(received.len(), 1);
    }

    #[test]
    fn test_broker_channel_not_found() {
        let mut broker = IpcBroker::new();
        let sender = test_agent_id(1);
        let receiver = test_agent_id(2);
        let msg = test_message(sender, receiver, "hello");

        let fake_id = ChannelId::new();
        assert!(matches!(broker.send(fake_id, msg), Err(IpcError::ChannelNotFound)));
    }
}

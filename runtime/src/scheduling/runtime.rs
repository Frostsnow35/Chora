//! Runtime — orchestrates the scheduling loop.
//!
//! The Runtime is the top-level coordinator that:
//! 1. Manages a process table of agents and their records.
//! 2. Uses a pluggable Scheduler to decide which agent runs next.
//! 3. Executes agent steps and updates state based on results.
//!
//! # Design
//!
//! The Runtime follows the mechanism/policy separation (P1):
//! - **Mechanism**: The Runtime provides the execution loop and state management.
//! - **Policy**: The Scheduler decides which agent runs next.
//!
//! The Runtime does not interpret scheduling decisions; it only executes them.
//!
//! # Example
//!
//! ```rust,no_run
//! use runtime::{Runtime, FifoScheduler, MockAgent, AgentId, Intent, IntentId};
//!
//! let mut runtime = Runtime::new(Box::new(FifoScheduler::new()));
//! let agent = MockAgent::default();
//! runtime.spawn(agent);
//!
//! // Main loop
//! while let Some(agent_id) = runtime.next_to_run() {
//!     let result = runtime.run_step(agent_id);
//!     // Handle result...
//! }
//! ```

use std::collections::HashMap;
use crate::{
    Agent, AgentId, AgentProgram, BlockReason, SchedulingState, Message,
    ipc::{IpcBroker, ChannelId, IpcError},
};
use crate::record::{AgentRecord, StateSnapshot};
use super::Scheduler;

/// The Runtime — orchestrates agent execution via scheduling.
///
/// Holds a process table (agent records), a scheduler policy, and an IPC broker.
/// The actual agent instances are stored separately to avoid borrow checker issues.
pub struct Runtime {
    /// Agent records (metadata, scheduling context, metrics).
    records: HashMap<AgentId, AgentRecord>,
    /// Scheduler policy (pluggable).
    scheduler: Box<dyn Scheduler>,
    /// IPC broker for inter-agent communication.
    ipc_broker: IpcBroker,
}

impl Runtime {
    /// Create a new Runtime with the given scheduler policy.
    pub fn new(scheduler: Box<dyn Scheduler>) -> Self {
        Self {
            records: HashMap::new(),
            scheduler,
            ipc_broker: IpcBroker::new(),
        }
    }

    /// Create a new Runtime with the given scheduler and IPC broker.
    pub fn with_ipc(scheduler: Box<dyn Scheduler>, ipc_broker: IpcBroker) -> Self {
        Self {
            records: HashMap::new(),
            scheduler,
            ipc_broker,
        }
    }

    /// Spawn a new agent into the runtime.
    ///
    /// The agent is added to the process table and marked as ready.
    /// Returns the agent's ID.
    pub fn spawn<A: Agent>(&mut self, agent: &A) -> AgentId {
        let id = agent.id();
        let record = AgentRecord::new(
            id,
            agent.intent().clone(),
            // Create a minimal AgentProgram (can be enhanced later)
            AgentProgram::new(
                crate::program::ModelDescriptor::new("mock", "v1"),
                crate::program::PromptDescriptor::new(""),
            ),
        );
        self.records.insert(id, record);
        self.scheduler.on_ready(id);
        id
    }

    /// Get the next agent to run (delegates to scheduler).
    pub fn next_to_run(&mut self) -> Option<AgentId> {
        self.scheduler.next_to_run()
    }

    /// Get a reference to an agent's record.
    pub fn get_record(&self, agent_id: AgentId) -> Option<&AgentRecord> {
        self.records.get(&agent_id)
    }

    /// Get a mutable reference to an agent's record.
    pub fn get_record_mut(&mut self, agent_id: AgentId) -> Option<&mut AgentRecord> {
        self.records.get_mut(&agent_id)
    }

    /// Update the state of an agent and notify the scheduler.
    ///
    /// This should be called after each agent step to keep the scheduler informed.
    pub fn update_state(&mut self, agent_id: AgentId, new_state: SchedulingState) {
        if let Some(record) = self.records.get_mut(&agent_id) {
            record.update_state(StateSnapshot::from(new_state.clone()));

            // Notify scheduler of state change
            match new_state {
                SchedulingState::Ready => self.scheduler.on_ready(agent_id),
                SchedulingState::Running => {} // No notification needed
                SchedulingState::Blocked { reason } => self.scheduler.on_block(agent_id, reason),
                SchedulingState::Suspended => self.scheduler.on_block(
                    agent_id,
                    BlockReason::Other("suspended".to_string()),
                ),
                SchedulingState::Terminated { reason, .. } => {
                    self.scheduler.on_terminate(agent_id, reason)
                }
            }
        }
    }

    /// Record that an agent completed a step.
    pub fn record_step(&mut self, agent_id: AgentId, tokens_used: u64) {
        if let Some(record) = self.records.get_mut(&agent_id) {
            record.record_step();
            record.record_tokens(tokens_used);
        }
    }

    /// Check if an agent's constraints are violated.
    pub fn check_constraints(&self, agent_id: AgentId) -> bool {
        self.records
            .get(&agent_id)
            .map(|r| r.check_constraints_violated())
            .unwrap_or(false)
    }

    /// Get the number of agents in the process table.
    pub fn agent_count(&self) -> usize {
        self.records.len()
    }

    /// Get the number of ready agents (from scheduler).
    pub fn ready_count(&self) -> usize {
        self.scheduler.ready_count()
    }

    /// Check if there are any ready agents.
    pub fn has_ready(&self) -> bool {
        self.scheduler.has_ready()
    }

    /// Get a reference to the scheduler (for advanced use).
    pub fn scheduler(&self) -> &dyn Scheduler {
        self.scheduler.as_ref()
    }

    /// Get a mutable reference to the scheduler (for advanced use).
    pub fn scheduler_mut(&mut self) -> &mut dyn Scheduler {
        self.scheduler.as_mut()
    }

    // ==================== IPC Methods ====================

    /// Get a reference to the IPC broker.
    pub fn ipc_broker(&self) -> &IpcBroker {
        &self.ipc_broker
    }

    /// Get a mutable reference to the IPC broker.
    pub fn ipc_broker_mut(&mut self) -> &mut IpcBroker {
        &mut self.ipc_broker
    }

    /// Create a P2P IPC channel between two agents.
    ///
    /// Returns the ChannelId on success.
    pub fn create_p2p_channel(
        &mut self,
        sender: AgentId,
        receiver: AgentId,
        capacity: usize,
    ) -> Result<ChannelId, IpcError> {
        self.ipc_broker.create_p2p_channel(sender, receiver, capacity)
    }

    /// Create a broadcast IPC channel.
    ///
    /// Returns the ChannelId on success.
    pub fn create_broadcast_channel(
        &mut self,
        sender: AgentId,
        receivers: Vec<AgentId>,
        capacity: usize,
    ) -> Result<ChannelId, IpcError> {
        self.ipc_broker.create_broadcast_channel(sender, receivers, capacity)
    }

    /// Send a message through an IPC channel.
    pub fn send_ipc(&mut self, channel_id: ChannelId, msg: Message) -> Result<(), IpcError> {
        let receiver = msg.receiver;
        let result = self.ipc_broker.send(channel_id, msg);

        // If message was sent successfully and receiver was blocked, wake it up
        if result.is_ok() {
            if let Some(record) = self.records.get(&receiver) {
                if matches!(record.state.current.as_str(), "Blocked") {
                    self.scheduler.on_ready(receiver);
                }
            }
        }

        result
    }

    /// Poll for pending IPC messages for an agent (from all channels).
    pub fn poll_ipc_messages(&mut self, agent_id: AgentId) -> Vec<Message> {
        self.ipc_broker.poll_all(agent_id)
    }

    /// Get the number of pending IPC messages for an agent.
    pub fn pending_ipc_count(&self, agent_id: AgentId) -> usize {
        self.ipc_broker.pending_count(agent_id)
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("agent_count", &self.records.len())
            .field("ready_count", &self.scheduler.ready_count())
            .field("scheduler", &format_args!("<{}>", std::any::type_name::<dyn Scheduler>()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockAgent, TerminationReason};

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        assert_eq!(runtime.agent_count(), 0);
        assert_eq!(runtime.ready_count(), 0);
    }

    #[test]
    fn test_runtime_spawn() {
        let mut runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        let agent = MockAgent::default();

        let id = runtime.spawn(&agent);
        assert_eq!(runtime.agent_count(), 1);
        assert_eq!(runtime.ready_count(), 1);
        assert!(runtime.get_record(id).is_some());
    }

    #[test]
    fn test_runtime_next_to_run() {
        let mut runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        let agent1 = MockAgent::default();
        let agent2 = MockAgent::default();

        let id1 = runtime.spawn(&agent1);
        let id2 = runtime.spawn(&agent2);

        // FIFO order
        assert_eq!(runtime.next_to_run(), Some(id1));
        assert_eq!(runtime.next_to_run(), Some(id2));
        assert_eq!(runtime.next_to_run(), None);
    }

    #[test]
    fn test_runtime_update_state() {
        let mut runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        let agent = MockAgent::default();

        let id = runtime.spawn(&agent);
        assert_eq!(runtime.ready_count(), 1);

        // Block the agent
        runtime.update_state(id, SchedulingState::Blocked {
            reason: BlockReason::WaitForMessage,
        });
        assert_eq!(runtime.ready_count(), 0);

        // Unblock the agent
        runtime.update_state(id, SchedulingState::Ready);
        assert_eq!(runtime.ready_count(), 1);
    }

    #[test]
    fn test_runtime_record_step() {
        let mut runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        let agent = MockAgent::default();

        let id = runtime.spawn(&agent);
        runtime.record_step(id, 100);

        let record = runtime.get_record(id).unwrap();
        assert_eq!(record.metrics.steps_executed, 1);
        assert_eq!(record.metrics.tokens_consumed, 100);
    }

    #[test]
    fn test_runtime_terminate() {
        let mut runtime = Runtime::new(Box::new(super::super::FifoScheduler::new()));
        let agent = MockAgent::default();

        let id = runtime.spawn(&agent);
        runtime.update_state(id, SchedulingState::Terminated {
            reason: TerminationReason::Success,
            at: chrono::Utc::now(),
        });

        assert_eq!(runtime.ready_count(), 0);
        assert_eq!(runtime.next_to_run(), None);
    }
}

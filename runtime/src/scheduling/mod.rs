//! Scheduling subsystem — RFC-002 implementation.
//!
//! The scheduler decides **which agent runs next** when multiple agents are
//! in `Ready` state. This module implements the **cooperative scheduling** model:
//! agents yield explicitly (via `yield_now()`), never preempted mid-step.
//!
//! # Design Principles
//!
//! - **P1 (Mechanism/Policy)**: The `Scheduler` trait exposes mechanisms;
//!   policies (FIFO, deadline-aware, token-budget) are pluggable.
//! - **O(1) hot path**: `next_to_run()`, `on_ready()`, `on_block()` are O(1).
//! - **Lazy deletion**: Blocked/terminated agents are removed lazily from
//!   the queue to keep `on_block()` O(1).
//!
//! # Architecture
//!
//! ```text
//! Scheduler trait (mechanism)
//! ├── FifoScheduler (policy: first-in-first-out)
//! ├── DeadlineAwareScheduler (policy: urgent deadlines first)
//! └── TokenBudgetScheduler (policy: more budget remaining → higher priority)
//! ```

mod fifo_scheduler;
mod runtime;

pub use fifo_scheduler::FifoScheduler;
pub use runtime::Runtime;

use crate::{AgentId, BlockReason, TerminationReason};
use crate::record::SchedulingContext;

/// External events that may affect scheduling decisions.
#[derive(Debug, Clone)]
pub enum SchedulingEvent {
    /// Timer fired (for time-based scheduling).
    TimerFired,
    /// Message arrived for a blocked agent.
    MessageArrived { target: AgentId },
    /// Tool result ready for a blocked agent.
    ToolResultReady { target: AgentId },
    /// Custom event (for extensibility).
    Custom(String),
}

/// Scheduler trait — the mechanism for selecting the next agent to run.
///
/// Implementations (policies) decide **which** agent runs next based on
/// their own criteria (FIFO, deadline, token budget, etc.). The runtime
/// only calls this trait; it does not interpret the policy.
///
/// # Performance
///
/// All methods should be O(1) to avoid becoming a bottleneck.
pub trait Scheduler: std::fmt::Debug + Send + Sync {
    /// Get the next agent to run, if any are ready.
    ///
    /// Returns `None` if no agents are in the ready queue.
    fn next_to_run(&mut self) -> Option<AgentId>;

    /// Signal that an agent has yielded (voluntarily gave up its turn).
    ///
    /// The agent should be re-added to the ready queue.
    fn on_yield(&mut self, agent_id: AgentId);

    /// Signal that an agent has become blocked.
    ///
    /// The agent should be removed from the ready queue.
    fn on_block(&mut self, agent_id: AgentId, reason: BlockReason);

    /// Signal that an agent has become ready (e.g., after resuming).
    ///
    /// The agent should be added to the ready queue.
    fn on_ready(&mut self, agent_id: AgentId);

    /// Signal that an agent has terminated.
    ///
    /// The agent should be removed from the ready queue permanently.
    fn on_terminate(&mut self, agent_id: AgentId, reason: TerminationReason);

    /// Handle an external event.
    ///
    /// Events may trigger scheduling decisions (e.g., message arrived →
    /// unblock the waiting agent).
    fn on_event(&mut self, event: SchedulingEvent);

    /// Update scheduling context for an agent (e.g., priority change).
    ///
    /// The scheduler may use this to adjust its internal state.
    fn update_agent_context(&mut self, agent_id: AgentId, context: &SchedulingContext);

    /// Get the number of agents currently in the ready queue.
    fn ready_count(&self) -> usize;

    /// Check if the scheduler has any ready agents.
    fn has_ready(&self) -> bool {
        self.ready_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a test AgentId.
    fn test_agent_id(n: u64) -> AgentId {
        // Use deterministic IDs for testing
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    // ========== FIFO Scheduler Tests ==========

    #[test]
    fn test_fifo_scheduler_empty() {
        let mut scheduler = FifoScheduler::new();
        assert_eq!(scheduler.ready_count(), 0);
        assert!(!scheduler.has_ready());
        assert!(scheduler.next_to_run().is_none());
    }

    #[test]
    fn test_fifo_scheduler_single_agent() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        scheduler.on_ready(agent);
        assert_eq!(scheduler.ready_count(), 1);
        assert!(scheduler.has_ready());

        let next = scheduler.next_to_run();
        assert_eq!(next, Some(agent));
        assert_eq!(scheduler.ready_count(), 0);
    }

    #[test]
    fn test_fifo_scheduler_fifo_order() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);

        // Add in order: 1, 2, 3
        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);
        scheduler.on_ready(agent3);

        assert_eq!(scheduler.ready_count(), 3);

        // Should come out in FIFO order
        assert_eq!(scheduler.next_to_run(), Some(agent1));
        assert_eq!(scheduler.next_to_run(), Some(agent2));
        assert_eq!(scheduler.next_to_run(), Some(agent3));
        assert_eq!(scheduler.next_to_run(), None);
    }

    #[test]
    fn test_fifo_scheduler_on_block() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);

        // Block agent1
        scheduler.on_block(agent1, BlockReason::WaitForMessage);

        // agent2 should still be runnable
        assert_eq!(scheduler.next_to_run(), Some(agent2));

        // agent1 should not be runnable (lazy deletion)
        assert_eq!(scheduler.next_to_run(), None);
    }

    #[test]
    fn test_fifo_scheduler_on_terminate() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);

        // Terminate agent1
        scheduler.on_terminate(agent1, TerminationReason::Success);

        // agent2 should still be runnable
        assert_eq!(scheduler.next_to_run(), Some(agent2));

        // agent1 should not be runnable
        assert_eq!(scheduler.next_to_run(), None);
    }

    #[test]
    fn test_fifo_scheduler_on_yield() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);

        // Consume agent1
        assert_eq!(scheduler.next_to_run(), Some(agent1));

        // agent1 yields (should be re-added to back of queue)
        scheduler.on_yield(agent1);

        // agent2 should run next, then agent1
        assert_eq!(scheduler.next_to_run(), Some(agent2));
        assert_eq!(scheduler.next_to_run(), Some(agent1));
    }

    #[test]
    fn test_fifo_scheduler_no_duplicates() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        // Add same agent twice
        scheduler.on_ready(agent);
        scheduler.on_ready(agent);

        // Should only appear once
        assert_eq!(scheduler.ready_count(), 1);
        assert_eq!(scheduler.next_to_run(), Some(agent));
        assert_eq!(scheduler.next_to_run(), None);
    }

    #[test]
    fn test_fifo_scheduler_on_event_no_op() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        scheduler.on_ready(agent);

        // Events are no-op for FIFO
        scheduler.on_event(SchedulingEvent::TimerFired);
        scheduler.on_event(SchedulingEvent::MessageArrived { target: agent });

        // Agent should still be runnable
        assert_eq!(scheduler.next_to_run(), Some(agent));
    }

    #[test]
    fn test_fifo_scheduler_update_context_no_op() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);
        let context = SchedulingContext::default();

        scheduler.on_ready(agent);

        // Context updates are no-op for FIFO
        scheduler.update_agent_context(agent, &context);

        // Agent should still be runnable
        assert_eq!(scheduler.next_to_run(), Some(agent));
    }

    #[test]
    fn test_fifo_scheduler_complex_scenario() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);

        // Initial setup: all ready
        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);
        scheduler.on_ready(agent3);

        // agent1 runs
        assert_eq!(scheduler.next_to_run(), Some(agent1));

        // agent2 gets blocked
        scheduler.on_block(agent2, BlockReason::WaitForToolResult);

        // agent3 runs
        assert_eq!(scheduler.next_to_run(), Some(agent3));

        // agent1 yields (re-added to queue)
        scheduler.on_yield(agent1);

        // agent1 runs again
        assert_eq!(scheduler.next_to_run(), Some(agent1));

        // agent2 unblocks (becomes ready again)
        scheduler.on_ready(agent2);

        // agent2 runs
        assert_eq!(scheduler.next_to_run(), Some(agent2));

        // Queue should be empty now
        assert_eq!(scheduler.next_to_run(), None);
        assert_eq!(scheduler.ready_count(), 0);
    }
}

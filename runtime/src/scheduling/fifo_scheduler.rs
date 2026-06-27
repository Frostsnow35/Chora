//! FIFO Scheduler — simplest scheduling policy.
//!
//! Agents run in the order they became ready (first-in-first-out).
//! This is the baseline policy for validation and comparison.
//!
//! # Performance
//!
//! All operations are O(1) amortized:
//! - `next_to_run()`: O(1) — pop from front, lazy-skip blocked agents
//! - `on_ready()`: O(1) — push to back, deduplicate via HashSet
//! - `on_block()` / `on_terminate()`: O(1) — mark as inactive (lazy deletion)
//! - `on_yield()`: O(1) — push to back (same as on_ready)
//!
//! # Lazy Deletion
//!
//! When an agent is blocked or terminated, we don't remove it from the queue
//! immediately (that would be O(n)). Instead, we mark it as "inactive" in a
//! HashSet. When `next_to_run()` pops an agent, it checks if the agent is
//! still active; if not, it skips it and tries the next one.
//!
//! This keeps all operations O(1) while ensuring correctness.

use std::collections::{HashSet, VecDeque};
use crate::{AgentId, BlockReason, TerminationReason};
use crate::record::SchedulingContext;
use super::{Scheduler, SchedulingEvent};

/// FIFO scheduler — agents run in the order they became ready.
#[derive(Debug)]
pub struct FifoScheduler {
    /// Queue of agent IDs (FIFO order).
    /// May contain "stale" entries (blocked/terminated agents) due to lazy deletion.
    queue: VecDeque<AgentId>,
    /// Set of currently active (ready) agents.
    /// Used for O(1) deduplication and lazy deletion checks.
    active: HashSet<AgentId>,
}

impl FifoScheduler {
    /// Create a new FIFO scheduler with an empty queue.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            active: HashSet::new(),
        }
    }
}

impl Default for FifoScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for FifoScheduler {
    fn next_to_run(&mut self) -> Option<AgentId> {
        // Pop from front, skipping stale entries (lazy deletion)
        while let Some(agent_id) = self.queue.pop_front() {
            if self.active.contains(&agent_id) {
                // Found a valid agent — remove from active set (it's now running)
                self.active.remove(&agent_id);
                return Some(agent_id);
            }
            // Stale entry — skip and try next
        }
        None
    }

    fn on_yield(&mut self, agent_id: AgentId) {
        // Agent yielded — re-add to back of queue
        if self.active.insert(agent_id) {
            self.queue.push_back(agent_id);
        }
    }

    fn on_block(&mut self, agent_id: AgentId, _reason: BlockReason) {
        // Mark as inactive (lazy deletion from queue)
        self.active.remove(&agent_id);
    }

    fn on_ready(&mut self, agent_id: AgentId) {
        // Add to active set and queue (deduplicate via HashSet)
        if self.active.insert(agent_id) {
            self.queue.push_back(agent_id);
        }
    }

    fn on_terminate(&mut self, agent_id: AgentId, _reason: TerminationReason) {
        // Mark as inactive (lazy deletion from queue)
        self.active.remove(&agent_id);
    }

    fn on_event(&mut self, _event: SchedulingEvent) {
        // FIFO ignores events (no-op)
    }

    fn update_agent_context(&mut self, _agent_id: AgentId, _context: &SchedulingContext) {
        // FIFO ignores priority/context changes (no-op)
    }

    fn ready_count(&self) -> usize {
        self.active.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    #[test]
    fn test_fifo_new() {
        let scheduler = FifoScheduler::new();
        assert_eq!(scheduler.ready_count(), 0);
        assert!(scheduler.queue.is_empty());
        assert!(scheduler.active.is_empty());
    }

    #[test]
    fn test_fifo_on_ready_adds_to_queue() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        scheduler.on_ready(agent);

        assert_eq!(scheduler.ready_count(), 1);
        assert!(scheduler.active.contains(&agent));
        assert_eq!(scheduler.queue.len(), 1);
    }

    #[test]
    fn test_fifo_on_ready_deduplicates() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        scheduler.on_ready(agent);
        scheduler.on_ready(agent); // duplicate

        assert_eq!(scheduler.ready_count(), 1);
        assert_eq!(scheduler.queue.len(), 1); // only one entry
    }

    #[test]
    fn test_fifo_next_to_run_pops_from_front() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);

        assert_eq!(scheduler.next_to_run(), Some(agent1));
        assert_eq!(scheduler.ready_count(), 1);
        assert!(!scheduler.active.contains(&agent1)); // removed from active
    }

    #[test]
    fn test_fifo_lazy_deletion_on_block() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);
        scheduler.on_ready(agent3);

        // Block agent1 (in middle of queue)
        scheduler.on_block(agent1, BlockReason::WaitForMessage);

        // Queue still has 3 entries (lazy deletion)
        assert_eq!(scheduler.queue.len(), 3);
        // But active set only has 2
        assert_eq!(scheduler.ready_count(), 2);

        // next_to_run should skip agent1
        assert_eq!(scheduler.next_to_run(), Some(agent2));
        assert_eq!(scheduler.next_to_run(), Some(agent3));
        assert_eq!(scheduler.next_to_run(), None);

        // Queue should now be empty (all stale entries popped)
        assert!(scheduler.queue.is_empty());
    }

    #[test]
    fn test_fifo_yield_re_adds_to_back() {
        let mut scheduler = FifoScheduler::new();
        let agent1 = test_agent_id(1);
        let agent2 = test_agent_id(2);

        scheduler.on_ready(agent1);
        scheduler.on_ready(agent2);

        // agent1 runs
        assert_eq!(scheduler.next_to_run(), Some(agent1));

        // agent1 yields (re-added to back)
        scheduler.on_yield(agent1);

        // agent2 should run next, then agent1
        assert_eq!(scheduler.next_to_run(), Some(agent2));
        assert_eq!(scheduler.next_to_run(), Some(agent1));
    }

    #[test]
    fn test_fifo_terminate_removes_permanently() {
        let mut scheduler = FifoScheduler::new();
        let agent = test_agent_id(1);

        scheduler.on_ready(agent);
        scheduler.on_terminate(agent, TerminationReason::Success);

        // Agent should not be runnable
        assert_eq!(scheduler.next_to_run(), None);
        assert_eq!(scheduler.ready_count(), 0);
    }
}

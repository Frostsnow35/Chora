//! A minimal mock agent for testing the Agent trait.
//!
//! This is not a production agent — it's designed to validate that the trait surface works correctly,
//! and to enable testing of the runtime without LLM dependencies.

use crate::{
    Agent, AgentId, AgentProgram, DeliveryError, Intent, IntentId, SchedulingState,
    StepContext, StepMetrics, StepOutput, StepResult, StepStatus, TerminationReason,
};

/// A simple mock agent that executes a predefined script.
#[derive(Debug)]
pub struct MockAgent {
    id: AgentId,
    state: SchedulingState,
    intent: Intent,
    script_index: usize,
    terminated: bool,
}

impl MockAgent {
    /// Create a new mock agent with a given intent.
    pub fn new(id: AgentId, intent: Intent, _program: AgentProgram) -> Self {
        Self {
            id,
            state: SchedulingState::Ready,
            intent,
            script_index: 0,
            terminated: false,
        }
    }

    /// Get the current script line being executed (for debugging).
    pub fn current_script_line(&self) -> &str {
        // Placeholder for future script support
        ""
    }

    /// Check if all constraints have been violated.
    pub fn check_constraints_violated(&self) -> bool {
        self.intent.check_constraints(
            &crate::metrics::AgentMetrics {
                steps_executed: self.script_index as u64,
                tokens_consumed: 0,
                messages_sent: 0,
                messages_received: 0,
                created_at: chrono::Utc::now(),
                last_stepped_at: None,
            },
        )
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        let id = AgentId::new();
        let intent_id = IntentId::new();
        let intent = Intent::new_root(intent_id, "Mock task", Some(id));
        let program = AgentProgram::new(
            crate::program::ModelDescriptor::new("mock", "v1"),
            crate::program::PromptDescriptor::new("You are a mock agent."),
        );
        Self::new(id, intent, program)
    }
}

impl Agent for MockAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn intent(&self) -> &Intent {
        &self.intent
    }

    fn state(&self) -> SchedulingState {
        self.state.clone()
    }

    fn step(&mut self, _ctx: &mut StepContext) -> StepResult {
        if self.terminated {
            return StepResult {
                status: StepStatus::Interrupted {
                    reason: crate::InterruptReason::ForcefullyTerminated,
                },
                output: StepOutput::Text("Agent already terminated".to_string()),
                wants_yield: false,
                metrics: StepMetrics {
                    tokens_used: 0,
                    wall_time_ms: 0,
                },
            };
        }

        // Check constraint violations
        if self.check_constraints_violated() {
            self.terminated = true;
            self.state = crate::SchedulingState::Terminated {
                reason: TerminationReason::ConstraintViolation,
                at: chrono::Utc::now(),
            };
            return StepResult {
                status: StepStatus::Interrupted {
                    reason: crate::InterruptReason::TokenLimitExceeded,
                },
                output: StepOutput::Text("Constraints violated".to_string()),
                wants_yield: false,
                metrics: StepMetrics {
                    tokens_used: 0,
                    wall_time_ms: 0,
                },
            };
        }

        // Simulate a simple step
        self.script_index += 1;

        StepResult {
            status: StepStatus::Success,
            output: StepOutput::Text(format!("Step {} completed", self.script_index)),
            wants_yield: false,
            metrics: StepMetrics {
                tokens_used: 10, // Mock token count
                wall_time_ms: 1, // Mock time
            },
        }
    }

    fn receive(&mut self, _msg: crate::Message) -> Result<(), DeliveryError> {
        // Mock agent just accepts any message
        Ok(())
    }

    fn yield_now(&mut self) {
        tracing::debug!(agent_id = %self.id, "yield_now called");
    }

    fn resume(&mut self) {
        tracing::debug!(agent_id = %self.id, "resume called");
    }

    fn terminate(&mut self, reason: TerminationReason) {
        self.terminated = true;
        tracing::info!(agent_id = %self.id, reason = ?reason, "agent terminated");
        self.state = crate::SchedulingState::Terminated {
            reason,
            at: chrono::Utc::now(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Constraints, SchedulingHint, TokenBudget, TokenCounter, WorkingMemoryView,
    };

    /// Minimal working memory view for tests.
    #[derive(Debug)]
    struct TestMemoryView {
        content: String,
    }

    impl WorkingMemoryView for TestMemoryView {
        fn peek(&self) -> &str {
            &self.content
        }

        fn utilization(&self) -> f32 {
            0.0
        }
    }

    /// Helper that runs a single step on a default MockAgent and returns the result.
    fn step_once(agent: &mut MockAgent) -> StepResult {
        let mem = TestMemoryView {
            content: String::new(),
        };
        let mut counter = TokenCounter::new(0);
        let mut ctx = StepContext {
            working_memory: &mem,
            memory_capabilities: &[],
            pending_messages: &[],
            now: chrono::Utc::now(),
            token_counter: &mut counter,
            scheduling_hint: SchedulingHint::Normal,
        };
        agent.step(&mut ctx)
    }

    #[test]
    fn test_default_agent_is_ready() {
        let agent = MockAgent::default();
        assert_eq!(agent.state(), SchedulingState::Ready);
        assert!(!agent.terminated);
        assert_eq!(agent.current_script_line(), "");
    }

    #[test]
    fn test_agent_id_is_stable() {
        let agent = MockAgent::default();
        let id1 = agent.id();
        let id2 = agent.id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_two_agents_have_different_ids() {
        let a = MockAgent::default();
        let b = MockAgent::default();
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn test_intent_goal_is_set() {
        let agent = MockAgent::default();
        assert_eq!(agent.intent().goal.as_str(), "Mock task");
        assert!(agent.intent().parent.is_none());
    }

    #[test]
    fn test_step_returns_success() {
        let mut agent = MockAgent::default();
        let result = step_once(&mut agent);

        assert!(matches!(result.status, StepStatus::Success));
        assert!(!result.wants_yield);
        assert_eq!(result.metrics.tokens_used, 10);
        assert_eq!(result.metrics.wall_time_ms, 1);
        if let StepOutput::Text(text) = &result.output {
            assert!(text.contains("Step 1 completed"));
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_step_increments_script_index() {
        let mut agent = MockAgent::default();
        let _ = step_once(&mut agent);
        let _ = step_once(&mut agent);
        let result = step_once(&mut agent);

        if let StepOutput::Text(text) = &result.output {
            assert!(text.contains("Step 3 completed"));
        } else {
            panic!("expected text output");
        }
    }

    #[test]
    fn test_terminate_sets_terminated_state() {
        let mut agent = MockAgent::default();
        agent.terminate(TerminationReason::Success);

        assert!(agent.terminated);
        match agent.state() {
            SchedulingState::Terminated { reason, .. } => {
                assert_eq!(reason, TerminationReason::Success);
            }
            other => panic!("expected Terminated state, got {:?}", other),
        }
    }

    #[test]
    fn test_step_after_terminate_returns_interrupted() {
        let mut agent = MockAgent::default();
        agent.terminate(TerminationReason::Failure);

        let result = step_once(&mut agent);

        match result.status {
            StepStatus::Interrupted {
                reason: crate::InterruptReason::ForcefullyTerminated,
            } => {}
            other => panic!("expected Interrupted(ForcefullyTerminated), got {:?}", other),
        }
    }

    #[test]
    fn test_constraint_violation_terminates_agent() {
        let id = AgentId::new();
        let intent_id = IntentId::new();
        let mut intent = Intent::new_root(intent_id, "Budget-constrained", Some(id));
        // Set token budget to 0 so any step triggers violation.
        intent.constraints = Constraints {
            token_budget: Some(TokenBudget::new(0)),
            deadline: None,
            step_budget: None,
            capability_bounds: vec![],
        };
        let program = AgentProgram::new(
            crate::program::ModelDescriptor::new("mock", "v1"),
            crate::program::PromptDescriptor::new("test"),
        );
        let mut agent = MockAgent::new(id, intent, program);

        // script_index=0, tokens_consumed=0 in metrics, budget=0 -> 0 >= 0 is a violation.
        assert!(agent.check_constraints_violated());

        let result = step_once(&mut agent);

        match result.status {
            StepStatus::Interrupted {
                reason: crate::InterruptReason::TokenLimitExceeded,
            } => {}
            other => panic!("expected TokenLimitExceeded, got {:?}", other),
        }
        assert!(agent.terminated);
        match agent.state() {
            SchedulingState::Terminated {
                reason: TerminationReason::ConstraintViolation,
                ..
            } => {}
            other => panic!("expected Terminated(ConstraintViolation), got {:?}", other),
        }
    }

    #[test]
    fn test_yield_and_resume_do_not_panic() {
        let mut agent = MockAgent::default();
        agent.yield_now();
        agent.resume();
        // Still ready after yield/resume.
        assert_eq!(agent.state(), SchedulingState::Ready);
    }

    #[test]
    fn test_receive_accepts_message() {
        let mut agent = MockAgent::default();
        let msg = crate::Message {
            sender: AgentId::new(),
            receiver: agent.id(),
            message_type: "ping".to_string(),
            payload: serde_json::Value::Null,
            metadata: crate::MessageMetadata {
                sent_at: chrono::Utc::now(),
                priority: 0,
                requires_ack: false,
            },
        };
        assert!(agent.receive(msg).is_ok());
    }

    #[test]
    fn test_custom_program_is_accepted() {
        let id = AgentId::new();
        let intent_id = IntentId::new();
        let intent = Intent::new_root(intent_id, "custom", None);
        let program = AgentProgram::new(
            crate::program::ModelDescriptor::new("openai", "gpt-4"),
            crate::program::PromptDescriptor::new("You are helpful.").with_rule("Be concise"),
        );
        let agent = MockAgent::new(id, intent, program);
        assert_eq!(agent.id(), id);
        assert_eq!(agent.state(), SchedulingState::Ready);
    }
}

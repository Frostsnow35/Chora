//! A minimal mock agent for testing the Agent trait.
//!
//! This is not a production agent — it's designed to validate that the trait surface works correctly,
//! and to enable testing of the runtime without LLM dependencies.
//!
//! # Scripted behaviour
//!
//! By default the agent counts steps ("Step 1 completed", "Step 2 completed"…).
//! For tests that need deterministic tool-call behaviour, use [`MockAgent::with_script`]
//! to load a sequence of [`ScriptedAction`]s. The agent plays them in order,
//! then falls back to the default counter when the script is exhausted.

use crate::{
    Agent, AgentId, AgentProgram, DeliveryError, Intent, IntentId, SchedulingState, StepContext,
    StepMetrics, StepOutput, StepResult, StepStatus, TerminationReason, ToolCallRequest,
    ToolResult,
};

/// One action in a deterministic script.
///
/// When a `MockAgent` is loaded with a script, each call to [`Agent::step`]
/// consumes the next action and produces the corresponding output.
#[derive(Debug, Clone)]
pub enum ScriptedAction {
    /// Emit a text output.
    Text(String),
    /// Request a tool call. The agent synthesises a stable `call_id` derived
    /// from the current script index.
    ToolCall {
        /// Tool name.
        tool_name: String,
        /// Tool arguments.
        args: serde_json::Value,
    },
    /// Request a yield from the runtime.
    Yield,
    /// Terminate the agent with a reason.
    Terminate(TerminationReason),
}

/// A simple mock agent that executes a predefined script.
#[derive(Debug)]
pub struct MockAgent {
    id: AgentId,
    state: SchedulingState,
    intent: Intent,
    script_index: usize,
    terminated: bool,
    /// Optional deterministic script. When empty, `step()` falls back to the
    /// default counter-based text output.
    script: Vec<ScriptedAction>,
    /// Tool results delivered via `StepContext::pending_tool_result`. The agent
    /// appends each non-`None` result here so tests can inspect what was
    /// received.
    tool_result_log: Vec<ToolResult>,
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
            script: vec![],
            tool_result_log: vec![],
        }
    }

    /// Builder: load a deterministic script into the agent.
    pub fn with_script(mut self, script: Vec<ScriptedAction>) -> Self {
        self.script = script;
        self
    }

    /// Read-only view of tool results received so far.
    pub fn tool_result_log(&self) -> &[ToolResult] {
        &self.tool_result_log
    }

    /// Get the current script line being executed (for debugging).
    pub fn current_script_line(&self) -> &str {
        if let Some(action) = self.script.get(self.script_index) {
            match action {
                ScriptedAction::Text(t) => t.as_str(),
                ScriptedAction::ToolCall { tool_name, .. } => tool_name.as_str(),
                ScriptedAction::Yield => "<yield>",
                ScriptedAction::Terminate(_) => "<terminate>",
            }
        } else {
            ""
        }
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

    /// Consume any pending tool result from the context and log it.
    fn absorb_tool_result(&mut self, ctx: &mut StepContext) {
        if let Some(result) = ctx.pending_tool_result.take() {
            tracing::debug!(
                agent_id = %self.id,
                call_id = %result.call_id,
                tool_name = %result.tool_name,
                "absorbed tool result"
            );
            self.tool_result_log.push(result);
        }
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

    fn step(&mut self, ctx: &mut StepContext) -> StepResult {
        // Always absorb any tool result the runtime handed us.
        self.absorb_tool_result(ctx);

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

        // Determine output: scripted action or default counter behaviour.
        let (output, wants_yield, consumed_script) =
            if let Some(action) = self.script.get(self.script_index) {
                let (out, yld) = match action {
                    ScriptedAction::Text(t) => (StepOutput::Text(t.clone()), false),
                    ScriptedAction::ToolCall { tool_name, args } => {
                        let call_id = format!("call-{}-{}", self.id, self.script_index);
                        (
                            StepOutput::ToolCall(ToolCallRequest {
                                call_id,
                                tool_name: tool_name.clone(),
                                args: args.clone(),
                            }),
                            true, // yielding while we wait for the tool result
                        )
                    }
                    ScriptedAction::Yield => (StepOutput::Text("yielding".to_string()), true),
                    ScriptedAction::Terminate(reason) => {
                        self.terminated = true;
                        self.state = crate::SchedulingState::Terminated {
                            reason: reason.clone(),
                            at: chrono::Utc::now(),
                        };
                        (StepOutput::Text("terminated".to_string()), false)
                    }
                };
                (out, yld, true)
            } else {
                // Default: counter-based text output.
                self.script_index += 1;
                (
                    StepOutput::Text(format!("Step {} completed", self.script_index)),
                    false,
                    false,
                )
            };

        // Advance the script pointer only if we consumed a scripted action.
        if consumed_script {
            self.script_index += 1;
        }

        StepResult {
            status: StepStatus::Success,
            output,
            wants_yield,
            metrics: StepMetrics {
                tokens_used: 10,
                wall_time_ms: 1,
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
    use crate::{Constraints, SchedulingHint, TokenBudget, TokenCounter, WorkingMemoryView};

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

    /// Helper that runs a single step on a MockAgent and returns the result.
    fn step_once(agent: &mut MockAgent) -> StepResult {
        step_once_with_result(agent, None)
    }

    /// Helper that runs a single step, delivering an optional tool result.
    fn step_once_with_result(agent: &mut MockAgent, result: Option<ToolResult>) -> StepResult {
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
            pending_tool_result: result,
        };
        agent.step(&mut ctx)
    }

    // --- Basic behaviour (unchanged from previous version) ----------------

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

        assert!(agent.check_constraints_violated());

        let result = step_once(&mut agent);

        match result.status {
            StepStatus::Interrupted {
                reason: crate::InterruptReason::TokenLimitExceeded,
            } => {}
            other => panic!("expected TokenLimitExceeded, got {:?}", other),
        }
        assert!(agent.terminated);
    }

    #[test]
    fn test_yield_and_resume_do_not_panic() {
        let mut agent = MockAgent::default();
        agent.yield_now();
        agent.resume();
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

    // --- Scripting behaviour ---------------------------------------------

    #[test]
    fn test_scripted_text_action() {
        let mut agent = MockAgent::default().with_script(vec![ScriptedAction::Text(
            "hello from script".to_string(),
        )]);
        let result = step_once(&mut agent);

        assert!(matches!(result.status, StepStatus::Success));
        assert!(!result.wants_yield);
        match &result.output {
            StepOutput::Text(t) => assert_eq!(t, "hello from script"),
            other => panic!("expected text output, got {:?}", other),
        }
    }

    #[test]
    fn test_scripted_tool_call_action() {
        let mut agent = MockAgent::default().with_script(vec![ScriptedAction::ToolCall {
            tool_name: "add".to_string(),
            args: serde_json::json!({"a": 3, "b": 4}),
        }]);
        let result = step_once(&mut agent);

        assert!(matches!(result.status, StepStatus::Success));
        assert!(result.wants_yield); // tool calls yield while waiting for result
        match &result.output {
            StepOutput::ToolCall(req) => {
                assert_eq!(req.tool_name, "add");
                assert_eq!(req.args, serde_json::json!({"a": 3, "b": 4}));
                assert!(!req.call_id.is_empty());
            }
            other => panic!("expected ToolCall output, got {:?}", other),
        }
    }

    #[test]
    fn test_tool_result_integration() {
        let mut agent = MockAgent::default().with_script(vec![
            ScriptedAction::ToolCall {
                tool_name: "add".to_string(),
                args: serde_json::json!({"a": 1, "b": 2}),
            },
            ScriptedAction::Text("done".to_string()),
        ]);

        // Step 1: produce a tool call
        let result = step_once(&mut agent);
        let call_id = match &result.output {
            StepOutput::ToolCall(req) => req.call_id.clone(),
            other => panic!("expected ToolCall, got {:?}", other),
        };

        // Step 2: deliver the tool result back via StepContext
        let tool_result = ToolResult {
            call_id: call_id.clone(),
            tool_name: "add".to_string(),
            outcome: crate::ToolOutcome::Success(serde_json::json!({"result": 3.0})),
        };
        let _ = step_once_with_result(&mut agent, Some(tool_result));

        // The agent should have logged the result
        assert_eq!(agent.tool_result_log().len(), 1);
        assert_eq!(agent.tool_result_log()[0].call_id, call_id);
        assert_eq!(agent.tool_result_log()[0].tool_name, "add");
    }

    #[test]
    fn test_script_fallback_to_text() {
        // Empty script → falls back to default counter behaviour.
        let mut agent = MockAgent::default();
        let result = step_once(&mut agent);
        match &result.output {
            StepOutput::Text(t) => assert!(t.contains("Step 1 completed")),
            other => panic!("expected text output, got {:?}", other),
        }
    }

    #[test]
    fn test_mixed_script() {
        let mut agent = MockAgent::default().with_script(vec![
            ScriptedAction::Text("first".to_string()),
            ScriptedAction::ToolCall {
                tool_name: "echo".to_string(),
                args: serde_json::json!({"x": 1}),
            },
            ScriptedAction::Text("third".to_string()),
        ]);

        let r1 = step_once(&mut agent);
        match &r1.output {
            StepOutput::Text(t) => assert_eq!(t, "first"),
            other => panic!("step 1: expected text, got {:?}", other),
        }

        let r2 = step_once(&mut agent);
        match &r2.output {
            StepOutput::ToolCall(req) => assert_eq!(req.tool_name, "echo"),
            other => panic!("step 2: expected tool call, got {:?}", other),
        }

        let r3 = step_once(&mut agent);
        match &r3.output {
            StepOutput::Text(t) => assert_eq!(t, "third"),
            other => panic!("step 3: expected text, got {:?}", other),
        }
    }
}

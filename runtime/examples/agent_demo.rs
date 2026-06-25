/// Demo showing the core Agent trait functionality.
use runtime::{
    Agent, AgentId, AgentProgram, BuiltinToolExecutor, Intent, IntentId, MockAgent, ModelDescriptor,
    PromptDescriptor, ScriptedAction, SchedulingHint, StepContext, StepOutput, TerminationReason,
    TokenCounter, ToolExecutor, ToolOutcome, WorkingMemoryView,
};

/// A simple memory view for testing.
#[derive(Debug)]
struct SimpleMem(String);

impl WorkingMemoryView for SimpleMem {
    fn peek(&self) -> &str {
        &self.0
    }

    fn utilization(&self) -> f32 {
        (self.0.len() as f32) / 1000.0 // Assume 1000 char capacity
    }
}

fn main() {
    println!("=== Agent Runtime Demo ===\n");

    // 1. Create an agent with a specific intent
    let agent_id = AgentId::new();
    let intent_id = IntentId::new();
    let mut agent = MockAgent::new(
        agent_id,
        Intent::new_root(intent_id, "Count to 5", Some(agent_id)),
        AgentProgram::new(
            ModelDescriptor::new("mock", "v1"),
            PromptDescriptor::new("You are a counting agent."),
        ),
    );

    println!("1. Created agent: {}", agent.id());
    println!("   Goal: {}", agent.intent().goal.as_str());
    println!("   Initial state: {:?}", agent.state());

    // 2. Set up execution context
    let mem = SimpleMem(String::new());
    let mut counter = TokenCounter::new(0);
    let mut ctx = StepContext {
        working_memory: &mem,
        memory_capabilities: &[],
        pending_messages: &[],
        now: chrono::Utc::now(),
        token_counter: &mut counter,
        scheduling_hint: SchedulingHint::Normal,
        pending_tool_result: None,
    };

    // 3. Execute multiple steps
    println!("\n2. Executing steps...");
    for i in 1..=6 {
        let result = agent.step(&mut ctx);

        let status_str = match result.status {
            runtime::StepStatus::Success => "Success",
            runtime::StepStatus::Failed { ref reason } => &format!("Failed({})", reason),
            runtime::StepStatus::Interrupted { .. } => "Interrupted",
        };

        let output_preview = match &result.output {
            StepOutput::Text(t) => &t[..t.len().min(40)],
            _ => "Other",
        };

        println!(
            "   Step {}: status={}, tokens={}, output='{}'",
            i, status_str, result.metrics.tokens_used, output_preview
        );

        if let runtime::SchedulingState::Terminated { reason, .. } = agent.state() {
            println!("   -> Agent terminated with reason: {:?}", reason);
            break;
        }
    }

    // 4. Test manual termination
    println!("\n3. Testing manual termination...");
    let mut agent2 = MockAgent::default();
    println!("   Before: {:?}", agent2.state());
    agent2.terminate(TerminationReason::Success);
    println!("   After: {:?}", agent2.state());

    // 5. Test message handling
    println!("\n4. Testing message delivery...");
    let msg = runtime::Message {
        sender: AgentId::new(),
        receiver: agent2.id(),
        message_type: "ping".to_string(),
        payload: serde_json::json!({"query": "hello"}),
        metadata: runtime::MessageMetadata {
            sent_at: chrono::Utc::now(),
            priority: 5,
            requires_ack: true,
        },
    };
    match agent2.receive(msg) {
        Ok(_) => println!("   Message delivered successfully"),
        Err(e) => println!("   Delivery failed: {}", e),
    }

    // 6. Test yield/resume cycle
    println!("\n5. Testing yield/resume cycle...");
    let mut agent3 = MockAgent::default();
    agent3.yield_now();
    println!("   After yield: {:?}", agent3.state());
    agent3.resume();
    println!("   After resume: {:?}", agent3.state());

    // 7. Demonstrate constraint violation
    println!("\n6. Testing constraint violations...");
    let id = AgentId::new();
    let intent_id = IntentId::new();
    let mut intent = Intent::new_root(intent_id, "Zero budget task", Some(id));
    intent.constraints.token_budget = Some(runtime::TokenBudget::new(0));

    let mut constrained_agent = MockAgent::new(
        id,
        intent,
        AgentProgram::new(
            ModelDescriptor::new("mock", "v1"),
            PromptDescriptor::new("Test"),
        ),
    );

    let result = constrained_agent.step(&mut ctx);
    println!("   Step with zero budget: {:?}", result.status);
    match constrained_agent.state() {
        runtime::SchedulingState::Terminated { reason, .. } => {
            println!("   Agent terminated with reason: {:?}", reason);
        }
        _ => println!("   Agent still running (unexpected)"),
    }

    // 8. Demonstrate tool call pipeline
    println!("\n7. Testing tool call pipeline...");
    demo_tool_call_pipeline();

    println!("\n=== Demo Complete ===");
}

/// Runs the tool-call pipeline demo: agent requests tool → runtime dispatches
/// → result flows back via StepContext → agent logs it.
fn demo_tool_call_pipeline() {
    let executor = BuiltinToolExecutor::new();
    let mem = SimpleMem(String::new());

    println!("   Registered tools: {:?}", executor.tool_names());

    let agent_id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(intent_id, "compute 10 + 32", Some(agent_id));
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "v1"),
        PromptDescriptor::new("calculator"),
    );

    // Script the agent to call "add" then emit a summary.
    let mut agent = MockAgent::new(agent_id, intent, program).with_script(vec![
        ScriptedAction::ToolCall {
            tool_name: "add".to_string(),
            args: serde_json::json!({"a": 10, "b": 32}),
        },
        ScriptedAction::Text("calculation complete".to_string()),
    ]);

    // Step 1: agent produces a tool call.
    let mut counter = TokenCounter::new(0);
    let mut ctx = StepContext {
        working_memory: &mem,
        memory_capabilities: &[],
        pending_messages: &[],
        now: chrono::Utc::now(),
        token_counter: &mut counter,
        scheduling_hint: SchedulingHint::Normal,
        pending_tool_result: None,
    };
    let r1 = agent.step(&mut ctx);
    match &r1.output {
        StepOutput::ToolCall(req) => {
            println!(
                "   Step 1: agent requested tool '{}' with args {}",
                req.tool_name, req.args
            );

            // Runtime dispatches the tool call.
            let tool_result = executor.execute(req);
            println!("   Runtime dispatched: {:?}", tool_result.outcome);

            // Step 2: deliver the result back via StepContext.
            let mut counter2 = TokenCounter::new(0);
            let mut ctx2 = StepContext {
                working_memory: &mem,
                memory_capabilities: &[],
                pending_messages: &[],
                now: chrono::Utc::now(),
                token_counter: &mut counter2,
                scheduling_hint: SchedulingHint::Normal,
                pending_tool_result: Some(tool_result.clone()),
            };
            let r2 = agent.step(&mut ctx2);
            match &r2.output {
                StepOutput::Text(t) => println!("   Step 2: agent says '{}'", t),
                _ => println!("   Step 2: {:?}", r2.output),
            }

            // Verify the agent logged the result.
            println!(
                "   Agent's tool result log has {} entries",
                agent.tool_result_log().len()
            );
            if let Some(logged) = agent.tool_result_log().first() {
                match &logged.outcome {
                    ToolOutcome::Success(v) => {
                        println!("   Logged result: success({})", v);
                    }
                    ToolOutcome::Error { code, message } => {
                        println!("   Logged result: error({}: {})", code, message);
                    }
                }
            }
        }
        other => {
            println!("   Unexpected output: {:?}", other);
        }
    }
}

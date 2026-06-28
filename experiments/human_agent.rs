//! Human-AI Collaboration Experiment
//! 
//! Demonstrates seamless human-AI collaboration through the Intent Negotiation Protocol.
//! A human product manager proposes a feature change, and AI agents evaluate and vote on it.

use runtime::{
    AgentId, GoalDescription, SovereigntyLevel,
    negotiation::{
        NegotiationEngine, PlanAmendment, InitialResponse, VoteDecision,
        Outcome,
    },
    sovereignty::{ExecutionPlan, Constitution},
};

struct HumanProductManager {
    id: AgentId,
    name: String,
    level: SovereigntyLevel,
    global_trust: f64,
}

impl HumanProductManager {
    fn new() -> Self {
        Self {
            id: AgentId::new(),
            name: "Human PM".to_string(),
            level: SovereigntyLevel::Level3,
            global_trust: 1.0,
        }
    }

    fn propose_change(&self, new_goal: &str) -> PlanAmendment {
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new(new_goal),
        }
    }
}

struct AiAgent {
    id: AgentId,
    name: String,
    level: SovereigntyLevel,
    global_trust: f64,
    constitution: Constitution,
    execution_plan: ExecutionPlan,
}

impl AiAgent {
    fn new(name: &str, level: SovereigntyLevel, global_trust: f64, purpose: &str, initial_goal: &str) -> Self {
        let id = AgentId::new();
        Self {
            id,
            name: name.to_string(),
            level,
            global_trust,
            constitution: Constitution::new(purpose),
            execution_plan: ExecutionPlan::new(initial_goal),
        }
    }
}

fn print_header(title: &str) {
    println!("\n{}", "=".repeat(70));
    println!("  {}", title);
    println!("{}", "=".repeat(70));
}

fn main() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║          Human-AI Collaboration Experiment                       ║");
    println!("║              Intent Negotiation Protocol                         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    print_header("Phase 1: Team Setup");

    let human = HumanProductManager::new();
    let architect = AiAgent::new(
        "AI Architect",
        SovereigntyLevel::Level3,
        0.92,
        "Design maintainable software architecture",
        "Implement microservices architecture",
    );
    let implementer = AiAgent::new(
        "AI Implementer",
        SovereigntyLevel::Level2,
        0.85,
        "Deliver working code on time",
        "Implement user authentication",
    );
    let tester = AiAgent::new(
        "AI Tester",
        SovereigntyLevel::Level1,
        0.80,
        "Ensure quality through testing",
        "Write unit tests for core features",
    );

    println!("\n  Team Members:");
    println!("  • {} ({}): Level {:?}, Global Trust: {:.2}", human.name, human.id, human.level, human.global_trust);
    println!("  • {} ({}): Level {:?}, Global Trust: {:.2}", architect.name, architect.id, architect.level, architect.global_trust);
    println!("  • {} ({}): Level {:?}, Global Trust: {:.2}", implementer.name, implementer.id, implementer.level, implementer.global_trust);
    println!("  • {} ({}): Level {:?}, Global Trust: {:.2}", tester.name, tester.id, tester.level, tester.global_trust);

    let mut engine = NegotiationEngine::new();

    print_header("Phase 2: Human Proposes Feature Change");

    let proposal = human.propose_change("Customer request: add dark mode");
    println!("\n  Human proposal: {:?}", proposal);
    println!("  Rationale: Customer requested dark mode for better UX accessibility");

    print_header("Phase 3: AI Agents Evaluate Proposal");

    let affected_agents = vec![architect.id, implementer.id, tester.id];
    
    let session_id = engine.create_session(
        human.id,
        human.level,
        human.global_trust,
        architect.id,
        affected_agents.clone(),
        proposal.clone(),
        "Customer requested dark mode for better UX".to_string(),
    ).unwrap();
    
    println!("\n  Negotiation session created: {}", session_id);

    let architect_response = InitialResponse::CounterProposal {
        amendment: PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Revised: implement dark mode incrementally"),
        },
        rationale: "Dark mode feasible but should be incremental to manage risk".to_string(),
    };
    engine.submit_response(session_id, architect.id, architect_response.clone()).unwrap();
    println!("\n  {} response: {:?}", architect.name, architect_response);

    let implementer_response = InitialResponse::Reject {
        rationale: "Estimated effort exceeds available capacity for full implementation".to_string(),
    };
    engine.submit_response(session_id, implementer.id, implementer_response.clone()).unwrap();
    println!("  {} response: {:?}", implementer.name, implementer_response);

    let tester_response = InitialResponse::Accept;
    engine.submit_response(session_id, tester.id, tester_response.clone()).unwrap();
    println!("  {} response: {:?}", tester.name, tester_response);

    print_header("Phase 4: Counter-Proposal Evaluation");

    if let InitialResponse::CounterProposal { amendment, rationale } = &architect_response {
        println!("\n  {} proposed counter: {}", architect.name, rationale);
        println!("  New proposal: {:?}", amendment);

        engine.resolve_counter_proposals(session_id).unwrap();
        let session = engine.get_session(session_id).unwrap();
        println!("\n  Counter-proposal resolution complete");
        println!("  Phase: {:?}", session.phase);
    }

    print_header("Phase 5: Final Voting");

    println!("\n  Vote weights: 0.6×bilateral trust + 0.4×global trust");
    println!("  Consensus threshold: ≥60%");

    engine.submit_vote(session_id, human.id, VoteDecision::Accept, "Proposer approves".to_string()).unwrap();
    println!("\n  {}: Accept", human.name);

    engine.submit_vote(session_id, architect.id, VoteDecision::Accept, "Architect approves counter-proposal".to_string()).unwrap();
    println!("  {}: Accept", architect.name);

    engine.submit_vote(session_id, implementer.id, VoteDecision::Accept, "Implementer approves incremental approach".to_string()).unwrap();
    println!("  {}: Accept", implementer.name);

    engine.submit_vote(session_id, tester.id, VoteDecision::Accept, "Tester approves".to_string()).unwrap();
    println!("  {}: Accept", tester.name);

    print_header("Phase 6: Negotiation Result");

    let outcome = engine.complete_session(session_id).unwrap();
    println!("\n  Outcome: {:?}", outcome);
    
    if let Outcome::Accepted { .. } = outcome {
        println!("\n  🎉 Consensus reached! Proposal approved.");
        println!("  ✅ Human-AI collaboration successful!");
    } else {
        println!("\n  ❌ Proposal rejected.");
    }

    print_header("Phase 7: Trust Evolution");

    println!("\n  Bilateral trust updates (A→B):");
    println!("  • {} → {}: {:.2}", architect.name, human.name, engine.get_bilateral_trust(architect.id, human.id));
    println!("  • {} → {}: {:.2}", implementer.name, human.name, engine.get_bilateral_trust(implementer.id, human.id));
    println!("  • {} → {}: {:.2}", tester.name, human.name, engine.get_bilateral_trust(tester.id, human.id));

    println!("\n{}", "=".repeat(70));
    println!("  Experiment Complete");
    println!("{}", "=".repeat(70));
    println!("\n  Key Takeaways:");
    println!("  1. Human can propose changes at Level 3");
    println!("  2. AI agents can counter-propose for better solutions");
    println!("  3. Weighted voting ensures fair representation");
    println!("  4. Trust evolves based on collaboration outcomes");
}
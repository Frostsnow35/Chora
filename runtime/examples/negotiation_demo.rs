/// Demo showing multi-agent negotiation with the Intent Negotiation Protocol.
///
/// This example demonstrates how Level 3 agents can propose ExecutionPlan amendments,
/// and how affected agents evaluate and vote on proposals.
use runtime::{
    AgentId, GoalDescription, SovereigntyLevel,
    negotiation::{
        NegotiationEngine, PlanAmendment, InitialResponse, VoteDecision,
        Outcome,
    },
};

fn main() {
    println!("=== Negotiation Protocol Demo ===\n");

    // 1. Create a negotiation engine
    let mut engine = NegotiationEngine::new();
    println!("1. Created NegotiationEngine");

    // 2. Create agents
    let architect_id = AgentId::new();
    let developer_id = AgentId::new();
    let tester_id = AgentId::new();

    println!("\n2. Agents:");
    println!("   Architect (Level 3): {}", architect_id);
    println!("   Developer (Level 2): {}", developer_id);
    println!("   Tester (Level 1): {}", tester_id);

    // 3. Level 3 agent creates a negotiation session
    println!("\n3. Architect proposes a change");
    
    let session_id = engine.create_session(
        architect_id,
        SovereigntyLevel::Level3,
        0.9,
        developer_id,
        vec![developer_id, tester_id],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Add real-time notifications"),
        },
        "Users need instant updates".to_string(),
    ).unwrap();
    
    println!("   Session created: {}", session_id);

    // 4. Affected agents respond
    println!("\n4. Agents evaluate the proposal");
    
    let dev_response = InitialResponse::CounterProposal {
        amendment: PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Add polling-based notifications first"),
        },
        rationale: "Real-time requires WebSocket infrastructure".to_string(),
    };
    engine.submit_response(session_id, developer_id, dev_response.clone()).unwrap();
    println!("   Developer: {:?}", dev_response);

    let tester_response = InitialResponse::Accept;
    engine.submit_response(session_id, tester_id, tester_response.clone()).unwrap();
    println!("   Tester: {:?}", tester_response);

    // 5. Resolve counter-proposals
    println!("\n5. Resolving counter-proposals");
    engine.resolve_counter_proposals(session_id).unwrap();
    let session = engine.get_session(session_id).unwrap();
    println!("   Phase: {:?}", session.phase);

    // 6. Final voting
    println!("\n6. Final voting");
    println!("   Vote weights: 0.6×bilateral + 0.4×global");
    
    engine.submit_vote(session_id, architect_id, VoteDecision::Accept, "Accept compromise".to_string()).unwrap();
    engine.submit_vote(session_id, developer_id, VoteDecision::Accept, "My counter-proposal".to_string()).unwrap();
    engine.submit_vote(session_id, tester_id, VoteDecision::Accept, "Accept".to_string()).unwrap();

    // 7. Complete negotiation
    println!("\n7. Negotiation result");
    let outcome = engine.complete_session(session_id).unwrap();
    println!("   Outcome: {:?}", outcome);
    
    match outcome {
        Outcome::Accepted => println!("   ✅ Consensus reached!"),
        Outcome::Rejected => println!("   ❌ Proposal rejected"),
    }

    // 8. Trust evolution
    println!("\n8. Trust evolution");
    println!("   Developer → Architect: {:.2}", engine.get_bilateral_trust(developer_id, architect_id));
    println!("   Tester → Architect: {:.2}", engine.get_bilateral_trust(tester_id, architect_id));

    println!("\n=== Demo Complete ===");
}
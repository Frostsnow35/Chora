//! Intent Negotiation Protocol — RFC-005 implementation.
//!
//! This module implements multi-sovereign agent collaboration through
//! genuine negotiation: Level 3 agents propose ExecutionPlan amendments,
//! affected agents evaluate and submit counter-proposals, and consensus
//! emerges through weighted supermajority voting.

pub mod types;
pub mod error;
pub mod trust;
pub mod collector;
pub mod afs_integration;

pub use types::*;
pub use error::*;
pub use trust::{CollaborationEvent, BilateralThresholds, DirectedTrustMeter};
pub use collector::{CollaborationCollector, GlobalTrustContribution};
pub use afs_integration::{ProposalFile, write_proposal_to_afs, read_proposal_from_afs, list_proposals_in_afs, write_bilateral_trust_to_afs, write_all_bilateral_trust_to_afs};

use std::collections::HashMap;
use std::time::Instant;
use crate::AgentId;
use crate::sovereignty::SovereigntyLevel;

/// Configuration for negotiation engine backpressure and rate limiting.
#[derive(Debug, Clone)]
pub struct NegotiationConfig {
    pub max_active_sessions: usize,
    pub default_timeout: std::time::Duration,
    pub rate_limit_per_agent: u32,
    pub rate_limit_window_secs: u64,
}

impl Default for NegotiationConfig {
    fn default() -> Self {
        Self {
            max_active_sessions: 100,
            default_timeout: std::time::Duration::from_secs(300),
            rate_limit_per_agent: 10,
            rate_limit_window_secs: 60,
        }
    }
}

/// Rate limit entry for an agent.
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

/// The negotiation engine orchestrates the full negotiation lifecycle.
///
/// Manages negotiation sessions, bilateral trust state, and the
/// collaboration collector for bilateral→global settlement.
pub struct NegotiationEngine {
    sessions: HashMap<SessionId, NegotiationSession>,
    bilateral_trust: HashMap<(AgentId, AgentId), f64>,
    collector: CollaborationCollector,
    config: NegotiationConfig,
    rate_limits: HashMap<AgentId, RateLimitEntry>,
}

impl NegotiationEngine {
    pub fn new() -> Self {
        Self::with_config(NegotiationConfig::default())
    }

    pub fn with_config(config: NegotiationConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            bilateral_trust: HashMap::new(),
            collector: CollaborationCollector::new(),
            config,
            rate_limits: HashMap::new(),
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &NegotiationConfig {
        &self.config
    }

    /// Get the number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Set bilateral trust score (source → target).
    pub fn set_bilateral_trust(&mut self, source: AgentId, target: AgentId, score: f64) {
        self.bilateral_trust.insert((source, target), score.clamp(0.0, 1.0));
    }

    /// Get bilateral trust score (source → target). Returns 0.0 if not set.
    pub fn get_bilateral_trust(&self, source: AgentId, target: AgentId) -> f64 {
        self.bilateral_trust
            .get(&(source, target))
            .copied()
            .unwrap_or(0.0)
    }

    /// Create a new negotiation session.
    ///
    /// Requires proposer to be Level 3. Takes trust snapshots at creation time.
    /// The proposer is automatically included as a voting participant.
    ///
    /// Enforces backpressure (max active sessions) and rate limiting per agent.
    pub fn create_session(
        &mut self,
        proposer: AgentId,
        proposer_level: SovereigntyLevel,
        proposer_global_trust: f64,
        target_agent: AgentId,
        affected_agents: Vec<AgentId>,
        amendment: PlanAmendment,
        rationale: String,
    ) -> Result<SessionId, NegotiationError> {
        if proposer_level < SovereigntyLevel::Level3 {
            return Err(NegotiationError::InsufficientLevel(proposer));
        }

        // Backpressure check: limit active sessions
        if self.sessions.len() >= self.config.max_active_sessions {
            return Err(NegotiationError::Backpressure(
                self.sessions.len(),
                self.config.max_active_sessions,
            ));
        }

        // Rate limit check per agent
        self.check_rate_limit(proposer)?;

        let now = Instant::now();
        let session_id = SessionId::new();
        let proposal_id = ProposalId::new();

        let mut all_participants = vec![proposer];
        for &agent in &affected_agents {
            if agent != proposer && !all_participants.contains(&agent) {
                all_participants.push(agent);
            }
        }

        let snapshots: Vec<TrustSnapshot> = all_participants
            .iter()
            .map(|&agent_id| {
                let bilateral = if agent_id == proposer {
                    1.0
                } else {
                    self.get_bilateral_trust(agent_id, proposer)
                };
                let global = if agent_id == proposer {
                    proposer_global_trust
                } else {
                    0.0
                };
                TrustSnapshot {
                    agent_id,
                    global_trust: global,
                    bilateral_to_proposer: bilateral,
                }
            })
            .collect();

        let session = NegotiationSession {
            id: session_id,
            proposal: AmendmentProposal {
                id: proposal_id,
                proposer,
                target_agent,
                amendment: amendment.clone(),
                rationale,
                timestamp: now,
                status: ProposalStatus::Distributed,
            },
            affected_agents: all_participants,
            phase: NegotiationPhase::ProposalDistributed,
            initial_responses: HashMap::new(),
            counter_proposals: Vec::new(),
            final_votes: HashMap::new(),
            active_amendment: amendment,
            consensus_threshold: 0.6,
            created_at: now,
            trust_snapshots: snapshots,
            timeout_duration: self.config.default_timeout,
            last_activity_at: now,
        };

        self.sessions.insert(session_id, session);
        Ok(session_id)
    }

    /// Get a reference to a session.
    pub fn get_session(&self, session_id: SessionId) -> Result<&NegotiationSession, NegotiationError> {
        self.sessions
            .get(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))
    }

    /// Set global trust snapshot for an agent in a session.
    /// Must be called before voting to ensure correct weight calculation.
    pub fn set_global_trust_snapshot(
        &mut self,
        session_id: SessionId,
        agent_id: AgentId,
        global_trust: f64,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if !session.affected_agents.contains(&agent_id) {
            return Err(NegotiationError::NotAParticipant(agent_id));
        }

        if let Some(snapshot) = session.trust_snapshots.iter_mut().find(|s| s.agent_id == agent_id) {
            snapshot.global_trust = global_trust.clamp(0.0, 1.0);
        }

        Ok(())
    }

    /// Submit an initial response from an affected agent (Phase 2).
    pub fn submit_response(
        &mut self,
        session_id: SessionId,
        agent_id: AgentId,
        response: InitialResponse,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if !session.affected_agents.contains(&agent_id) {
            return Err(NegotiationError::NotAParticipant(agent_id));
        }

        if session.initial_responses.contains_key(&agent_id) {
            return Err(NegotiationError::DuplicateResponse(agent_id));
        }

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        session.initial_responses.insert(agent_id, response);
        session.last_activity_at = Instant::now();

        // Transition to EvaluationInProgress on first response
        if session.phase == NegotiationPhase::ProposalDistributed {
            session.phase = NegotiationPhase::EvaluationInProgress;
        }

        Ok(())
    }

    /// Resolve counter-proposals and transition to FinalVoting (Phase 3→4).
    ///
    /// If multiple counter-proposals exist, ranks by supporter count
    /// (agents who didn't Reject the original). Top-ranked becomes active amendment.
    pub fn resolve_counter_proposals(
        &mut self,
        session_id: SessionId,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        // Collect counter-proposals from responses
        let mut counter_proposals: Vec<CounterProposal> = Vec::new();
        for (&agent_id, response) in &session.initial_responses {
            if let InitialResponse::CounterProposal { amendment, rationale } = response {
                counter_proposals.push(CounterProposal {
                    proposer: agent_id,
                    amendment: amendment.clone(),
                    rationale: rationale.clone(),
                    supporters: Vec::new(),
                });
            }
        }

        if counter_proposals.is_empty() {
            // No counter-proposals: proceed with original amendment
            session.phase = NegotiationPhase::FinalVoting;
            session.proposal.status = ProposalStatus::FinalVoting;
            return Ok(());
        }

        // Count supporters for each counter-proposal:
        // supporters = agents who Accept or Abstain (i.e., didn't Reject)
        for cp in &mut counter_proposals {
            for (&agent_id, response) in &session.initial_responses {
                if agent_id == cp.proposer {
                    continue;
                }
                match response {
                    InitialResponse::Accept | InitialResponse::Abstain => {
                        cp.supporters.push(agent_id);
                    }
                    InitialResponse::Reject { .. } => {
                        // Reject = does not support any counter-proposal
                    }
                    InitialResponse::CounterProposal { .. } => {
                        // Other counter-proposers don't count as supporters
                    }
                }
            }
        }

        // Sort by supporter count (descending), stable sort preserves insertion order for ties
        counter_proposals.sort_by(|a, b| b.supporters.len().cmp(&a.supporters.len()));

        // Top-ranked counter-proposal becomes the active amendment
        let winner = counter_proposals.remove(0);
        session.active_amendment = winner.amendment;
        session.counter_proposals = counter_proposals;
        session.phase = NegotiationPhase::CounterProposalResolved;
        session.proposal.status = ProposalStatus::CounterProposalReceived;

        // Immediately transition to FinalVoting
        session.phase = NegotiationPhase::FinalVoting;
        session.proposal.status = ProposalStatus::FinalVoting;

        Ok(())
    }

    /// Submit a final vote from an affected agent (Phase 4).
    pub fn submit_vote(
        &mut self,
        session_id: SessionId,
        agent_id: AgentId,
        decision: VoteDecision,
        rationale: String,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if !session.affected_agents.contains(&agent_id) {
            return Err(NegotiationError::NotAParticipant(agent_id));
        }

        if session.final_votes.contains_key(&agent_id) {
            return Err(NegotiationError::DuplicateVote(agent_id));
        }

        if session.phase != NegotiationPhase::FinalVoting {
            return Err(NegotiationError::InvalidPhaseTransition(
                format!("Cannot vote in phase {:?}", session.phase),
            ));
        }

        // Compute weight: 0.6 × bilateral(agent→proposer) + 0.4 × global_trust(agent)
        let snapshot = session
            .trust_snapshots
            .iter()
            .find(|s| s.agent_id == agent_id)
            .map(|s| (s.bilateral_to_proposer, s.global_trust))
            .unwrap_or((0.0, 0.0));

        let weight = 0.6 * snapshot.0 + 0.4 * snapshot.1;

        let vote = Vote {
            agent_id,
            decision,
            rationale,
            weight,
            timestamp: Instant::now(),
        };

        session.final_votes.insert(agent_id, vote);
        session.last_activity_at = Instant::now();
        Ok(())
    }

    /// Complete the session: tally votes, determine outcome, record trust events.
    ///
    /// Returns the Outcome (Accepted or Rejected).
    pub fn complete_session(
        &mut self,
        session_id: SessionId,
    ) -> Result<Outcome, NegotiationError> {
        let session = self.sessions
            .get(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        // Calculate consensus
        let mut accept_weight = 0.0;
        let mut total_weight = 0.0;

        for vote in session.final_votes.values() {
            total_weight += vote.weight;
            if vote.decision == VoteDecision::Accept {
                accept_weight += vote.weight;
            }
        }

        let outcome = if total_weight > 0.0 && (accept_weight / total_weight) >= session.consensus_threshold {
            Outcome::Accepted
        } else {
            Outcome::Rejected
        };

        // Record collaboration events for bilateral trust and settlement
        let proposer = session.proposal.proposer;
        let proposal_id = session.proposal.id;

        // Collect vote decisions first to avoid borrow conflict
        let vote_decisions: Vec<(AgentId, VoteDecision)> = session
            .final_votes
            .values()
            .map(|v| (v.agent_id, v.decision))
            .collect();

        for (agent_id, decision) in vote_decisions {
            let event = match decision {
                VoteDecision::Accept => CollaborationEvent::ProposalAccepted { proposal_id },
                VoteDecision::Reject => CollaborationEvent::ProposalRejected { proposal_id },
                VoteDecision::Abstain => continue,
            };

            // Update bilateral trust: agent → proposer
            let current = self.get_bilateral_trust(agent_id, proposer);
            let impact = match &event {
                CollaborationEvent::ProposalAccepted { .. } => 0.05,
                CollaborationEvent::ProposalRejected { .. } => -0.01,
                _ => 0.0,
            };
            self.set_bilateral_trust(agent_id, proposer, current + impact);

            // Add to collector for global settlement
            self.collector.add_event(agent_id, event);
        }

        // Update session phase
        let session = self.sessions.get_mut(&session_id).unwrap();
        session.phase = NegotiationPhase::Completed(outcome);
        session.proposal.status = match outcome {
            Outcome::Accepted => ProposalStatus::Accepted,
            Outcome::Rejected => ProposalStatus::Rejected,
        };

        Ok(outcome)
    }

    /// Settle all pending collaboration events to global trust contributions.
    ///
    /// Returns capped contributions per agent. Call this after complete_session
    /// and apply the deltas to each agent's global TrustMeter.
    pub fn settle_to_global(&mut self) -> Vec<GlobalTrustContribution> {
        self.collector.settle()
    }

    /// Abort a session with a reason.
    pub fn abort_session(
        &mut self,
        session_id: SessionId,
        reason: AbortReason,
    ) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Err(NegotiationError::SessionEnded);
        }

        session.phase = NegotiationPhase::Aborted(reason);
        session.proposal.status = ProposalStatus::Aborted;
        Ok(())
    }

    /// Check if an agent has exceeded the session creation rate limit.
    fn check_rate_limit(&mut self, agent_id: AgentId) -> Result<(), NegotiationError> {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(self.config.rate_limit_window_secs);

        let entry = self.rate_limits.entry(agent_id).or_insert_with(|| RateLimitEntry {
            count: 0,
            window_start: now,
        });

        if now - entry.window_start > window {
            entry.window_start = now;
            entry.count = 0;
        }

        if entry.count >= self.config.rate_limit_per_agent {
            return Err(NegotiationError::RateLimited(agent_id, self.config.rate_limit_per_agent));
        }

        entry.count += 1;
        Ok(())
    }

    /// Check if a session has timed out based on last activity.
    pub fn check_timeout(&self, session_id: SessionId) -> Result<bool, NegotiationError> {
        let session = self.sessions
            .get(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;

        if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
            return Ok(false);
        }

        Ok(Instant::now() - session.last_activity_at > session.timeout_duration)
    }

    /// Check all active sessions for timeout and abort those that have timed out.
    /// Returns the number of sessions that were aborted due to timeout.
    pub fn check_all_timeouts(&mut self) -> usize {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for (&session_id, session) in &self.sessions {
            if matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_)) {
                continue;
            }
            if now - session.last_activity_at > session.timeout_duration {
                timed_out.push(session_id);
            }
        }

        let count = timed_out.len();
        for session_id in timed_out {
            let _ = self.abort_session(session_id, AbortReason::TotalTimeout);
        }

        count
    }

    /// Update the last activity timestamp for a session.
    pub fn update_last_activity(&mut self, session_id: SessionId) -> Result<(), NegotiationError> {
        let session = self.sessions
            .get_mut(&session_id)
            .ok_or(NegotiationError::SessionNotFound(session_id.to_string()))?;
        session.last_activity_at = Instant::now();
        Ok(())
    }

    /// Check if the engine is under backpressure (active sessions near limit).
    /// Returns the current load factor (0.0 to 1.0), where 1.0 means full capacity.
    pub fn backpressure_load(&self) -> f64 {
        self.sessions.len() as f64 / self.config.max_active_sessions as f64
    }

    /// Clean up completed/aborted sessions to free capacity.
    /// Returns the number of sessions cleaned up.
    pub fn cleanup_completed(&mut self) -> usize {
        let initial_count = self.sessions.len();
        self.sessions.retain(|_, session| {
            !matches!(session.phase, NegotiationPhase::Completed(_) | NegotiationPhase::Aborted(_))
        });
        initial_count - self.sessions.len()
    }

    // ─── IPC Integration ────────────────────────────────────────────

    /// Handle an IPC negotiation message.
    ///
    /// Receives a NegotiationMessage from IPC and processes it.
    /// Returns an optional response message to send back.
    ///
    /// Note: NegotiationEngine does NOT directly send IPC messages.
    /// It generates messages for the caller (Runtime) to send via IpcBroker.
    pub fn handle_ipc_message(
        &mut self,
        msg: crate::ipc::NegotiationMessage,
    ) -> Result<Option<crate::ipc::NegotiationMessage>, NegotiationError> {
        use crate::ipc::NegotiationMessage;

        match msg {
            NegotiationMessage::ResponseMessage {
                session_id,
                responder,
                response,
            } => {
                self.handle_response_from_ipc(session_id, responder, response)?;
                Ok(None)
            }

            NegotiationMessage::VoteMessage {
                session_id,
                voter,
                decision,
                rationale,
                weight: _, // weight is computed by engine, not from IPC
            } => {
                self.handle_vote_from_ipc(session_id, voter, decision, rationale)?;
                Ok(None)
            }

            NegotiationMessage::ProposalBroadcast { .. } => {
                // Broadcast messages are received by affected agents,
                // not processed by the engine itself
                Err(NegotiationError::InvalidPhaseTransition(
                    "ProposalBroadcast is for agents to receive, not for engine to process".to_string(),
                ))
            }

            NegotiationMessage::VotingStarted { .. } => {
                // VotingStarted is a notification to agents
                Err(NegotiationError::InvalidPhaseTransition(
                    "VotingStarted is for agents to receive, not for engine to process".to_string(),
                ))
            }

            NegotiationMessage::NegotiationResult { .. } => {
                // NegotiationResult is a notification to agents
                Err(NegotiationError::InvalidPhaseTransition(
                    "NegotiationResult is for agents to receive, not for engine to process".to_string(),
                ))
            }

            NegotiationMessage::NegotiationAborted { .. } => {
                // NegotiationAborted is a notification to agents
                Err(NegotiationError::InvalidPhaseTransition(
                    "NegotiationAborted is for agents to receive, not for engine to process".to_string(),
                ))
            }
        }
    }

    /// Handle a ResponseMessage from IPC (Phase 2).
    ///
    /// Converts InitialResponsePayload to InitialResponse and submits it.
    fn handle_response_from_ipc(
        &mut self,
        session_id: SessionId,
        responder: AgentId,
        response: crate::ipc::InitialResponsePayload,
    ) -> Result<(), NegotiationError> {
        // Convert IPC payload to internal type
        let internal_response: InitialResponse = response.into();
        self.submit_response(session_id, responder, internal_response)
    }

    /// Handle a VoteMessage from IPC (Phase 4).
    ///
    /// The weight field is ignored as it's computed by the engine.
    fn handle_vote_from_ipc(
        &mut self,
        session_id: SessionId,
        voter: AgentId,
        decision: VoteDecision,
        rationale: String,
    ) -> Result<(), NegotiationError> {
        self.submit_vote(session_id, voter, decision, rationale)
    }

    /// Create a negotiation session from a CreateProposalRequest (IPC).
    ///
    /// Returns the SessionId and a list of ProposalBroadcast messages
    /// to send to each affected agent.
    pub fn create_session_from_ipc(
        &mut self,
        request: crate::ipc::CreateProposalRequest,
        proposer_level: SovereigntyLevel,
        proposer_global_trust: f64,
    ) -> Result<(SessionId, Vec<crate::ipc::NegotiationMessage>), NegotiationError> {
        let session_id = self.create_session(
            request.proposer,
            proposer_level,
            proposer_global_trust,
            request.target_agent,
            request.affected_agents.clone(),
            request.amendment.clone(),
            request.rationale.clone(),
        )?;

        // Generate broadcast messages for all affected agents
        let broadcast_msgs = self.broadcast_proposal(session_id);

        Ok((session_id, broadcast_msgs))
    }

    /// Generate ProposalBroadcast messages for all affected agents.
    ///
    /// Returns a list of NegotiationMessage::ProposalBroadcast for the
    /// caller (Runtime) to send via IpcBroker to each affected agent.
    pub fn broadcast_proposal(&self, session_id: SessionId) -> Vec<crate::ipc::NegotiationMessage> {
        use crate::ipc::NegotiationMessage;

        let session = self.sessions.get(&session_id);
        if session.is_none() {
            return Vec::new();
        }

        let session = session.unwrap();
        let mut messages = Vec::new();

        for &agent_id in &session.affected_agents {
            if agent_id == session.proposal.proposer {
                continue; // Don't send to proposer
            }

            let msg = NegotiationMessage::ProposalBroadcast {
                session_id,
                proposer: session.proposal.proposer,
                target_agent: session.proposal.target_agent,
                amendment: session.proposal.amendment.clone(),
                rationale: session.proposal.rationale.clone(),
                affected_agents: session.affected_agents.clone(),
            };

            messages.push(msg);
        }

        messages
    }

    /// Generate VotingStarted messages for all affected agents.
    ///
    /// Call this after resolve_counter_proposals to notify agents
    /// that voting has begun. Returns messages for Runtime to send.
    pub fn broadcast_voting_started(&self, session_id: SessionId) -> Vec<crate::ipc::NegotiationMessage> {
        use crate::ipc::NegotiationMessage;

        let session = self.sessions.get(&session_id);
        if session.is_none() {
            return Vec::new();
        }

        let session = session.unwrap();

        if session.phase != NegotiationPhase::FinalVoting {
            return Vec::new();
        }

        let mut messages = Vec::new();

        for &agent_id in &session.affected_agents {
            if agent_id == session.proposal.proposer {
                continue; // Don't send to proposer (they know)
            }

            let msg = NegotiationMessage::VotingStarted {
                session_id,
                final_amendment: session.active_amendment.clone(),
            };

            messages.push(msg);
        }

        messages
    }

    /// Generate NegotiationResult messages for all affected agents.
    ///
    /// Call this after complete_session to notify agents of the outcome.
    pub fn broadcast_result(&self, session_id: SessionId) -> Option<crate::ipc::NegotiationMessage> {
        use crate::ipc::NegotiationMessage;

        let session = self.sessions.get(&session_id);
        if session.is_none() {
            return None;
        }

        let session = session.unwrap();

        match session.phase {
            NegotiationPhase::Completed(outcome) => {
                Some(NegotiationMessage::NegotiationResult {
                    session_id,
                    outcome,
                    final_amendment: Some(session.active_amendment.clone()),
                })
            }
            _ => None,
        }
    }

    /// Generate NegotiationAborted message for all affected agents.
    ///
    /// Call this after abort_session to notify agents.
    pub fn broadcast_abort(&self, session_id: SessionId) -> Option<crate::ipc::NegotiationMessage> {
        use crate::ipc::NegotiationMessage;

        let session = self.sessions.get(&session_id);
        if session.is_none() {
            return None;
        }

        let session = session.unwrap();

        match &session.phase {
            NegotiationPhase::Aborted(reason) => {
                Some(NegotiationMessage::NegotiationAborted {
                    session_id,
                    reason: reason.clone(),
                })
            }
            _ => None,
        }
    }
}

impl Default for NegotiationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentId;
    use crate::GoalDescription;
    use crate::sovereignty::SovereigntyLevel;
    use crate::ipc::{NegotiationMessage, CreateProposalRequest, InitialResponsePayload};

    fn test_agent_id(n: u64) -> AgentId {
        let bytes = n.to_le_bytes();
        let mut uuid_bytes = [0u8; 16];
        uuid_bytes[..8].copy_from_slice(&bytes);
        AgentId::from_bytes(uuid_bytes)
    }

    fn create_test_session(
        engine: &mut NegotiationEngine,
        proposer: AgentId,
        proposer_level: SovereigntyLevel,
        target: AgentId,
        affected: Vec<AgentId>,
        amendment: PlanAmendment,
    ) -> SessionId {
        engine.create_session(
            proposer,
            proposer_level,
            0.9,
            target,
            affected,
            amendment,
            "Test rationale".to_string(),
        ).unwrap()
    }

    #[test]
    fn test_engine_creation() {
        let engine = NegotiationEngine::new();
        assert_eq!(engine.session_count(), 0);
    }

    #[test]
    fn test_create_session_requires_level3() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let result = engine.create_session(
            proposer,
            SovereigntyLevel::Level2,
            0.5,
            target,
            affected,
            PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            "Requirement changed".to_string(),
        );

        assert!(matches!(result, Err(NegotiationError::InsufficientLevel(_))));
    }

    #[test]
    fn test_create_session_level3_succeeds() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let result = engine.create_session(
            proposer,
            SovereigntyLevel::Level3,
            0.9,
            target,
            affected,
            PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            "Requirement changed".to_string(),
        );

        assert!(result.is_ok());
        assert_eq!(engine.session_count(), 1);
    }

    #[test]
    fn test_proposer_is_in_affected_agents() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            target,
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        let session = engine.get_session(session_id).unwrap();
        assert!(session.affected_agents.contains(&proposer));
        assert_eq!(session.affected_agents.len(), 3);
    }

    #[test]
    fn test_submit_response() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.initial_responses.len(), 1);
    }

    #[test]
    fn test_submit_response_not_participant() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let outsider = test_agent_id(99);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        let result = engine.submit_response(session_id, outsider, InitialResponse::Accept);
        assert!(matches!(result, Err(NegotiationError::NotAParticipant(_))));
    }

    #[test]
    fn test_duplicate_response_rejected() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        let result = engine.submit_response(session_id, voter, InitialResponse::Reject {
            rationale: "Changed mind".to_string(),
        });
        assert!(matches!(result, Err(NegotiationError::DuplicateResponse(_))));
    }

    #[test]
    fn test_resolve_no_counter_proposals() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, affected[0], InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.phase, NegotiationPhase::FinalVoting);
    }

    #[test]
    fn test_resolve_single_counter_proposal() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Original") },
        );

        engine.submit_response(session_id, affected[0], InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("Modified"),
            },
            rationale: "Better approach".to_string(),
        }).unwrap();

        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.phase, NegotiationPhase::FinalVoting);
        match &session.active_amendment {
            PlanAmendment::ModifyGoal { new_goal } => {
                assert_eq!(new_goal.as_str(), "Modified");
            }
            _ => panic!("Expected ModifyGoal"),
        }
    }

    #[test]
    fn test_vote_weight_formula() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.8);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Good".to_string()).unwrap();

        let session = engine.get_session(session_id).unwrap();
        let vote = session.final_votes.get(&voter).unwrap();
        assert!((vote.weight - 0.48).abs() < 0.001);
    }

    #[test]
    fn test_consensus_supermajority_accepted() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        engine.set_bilateral_trust(agent2, proposer, 0.9);
        engine.set_bilateral_trust(agent3, proposer, 0.9);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, agent2, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, proposer, VoteDecision::Accept, "Proposer yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);
    }

    #[test]
    fn test_consensus_supermajority_rejected() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        engine.set_bilateral_trust(agent2, proposer, 0.9);
        engine.set_bilateral_trust(agent3, proposer, 0.9);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, agent2, VoteDecision::Reject, "No".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Reject, "No".to_string()).unwrap();
        engine.submit_vote(session_id, proposer, VoteDecision::Accept, "Proposer yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Rejected);
    }

    #[test]
    fn test_abstain_counts_in_denominator() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        engine.set_bilateral_trust(agent2, proposer, 0.8);
        engine.set_bilateral_trust(agent3, proposer, 0.8);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(4),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        engine.submit_vote(session_id, agent2, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, agent3, VoteDecision::Abstain, "".to_string()).unwrap();
        engine.submit_vote(session_id, proposer, VoteDecision::Abstain, "".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Rejected);
    }

    #[test]
    fn test_settlement_updates_bilateral_trust() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.5);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.complete_session(session_id).unwrap();

        let trust = engine.get_bilateral_trust(voter, proposer);
        assert!((trust - 0.55).abs() < 0.001);
    }

    #[test]
    fn test_settlement_returns_global_contributions() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.5);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);

        let contributions = engine.settle_to_global();
        assert!(!contributions.is_empty());
    }

    #[test]
    fn test_abort_session() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(2),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.abort_session(session_id, AbortReason::ProposerTerminated).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert!(matches!(
            session.phase,
            NegotiationPhase::Aborted(AbortReason::ProposerTerminated)
        ));
    }

    #[test]
    fn test_global_trust_snapshot() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.5);

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New") },
        );

        engine.set_global_trust_snapshot(session_id, voter, 0.7).unwrap();

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();

        let session = engine.get_session(session_id).unwrap();
        let vote = session.final_votes.get(&voter).unwrap();
        // weight = 0.6 * 0.5 + 0.4 * 0.7 = 0.3 + 0.28 = 0.58
        assert!((vote.weight - 0.58).abs() < 0.001);
    }

    #[test]
    fn test_multiple_counter_proposals_ranked_by_supporters() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let agent4 = test_agent_id(4);
        let agent5 = test_agent_id(5);
        let affected = vec![agent2, agent3, agent4, agent5];

        let session_id = create_test_session(
            &mut engine,
            proposer, SovereigntyLevel::Level3, test_agent_id(6),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Original") },
        );

        engine.submit_response(session_id, agent2, InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("CounterA"),
            },
            rationale: "Approach A".to_string(),
        }).unwrap();

        engine.submit_response(session_id, agent3, InitialResponse::CounterProposal {
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("CounterB"),
            },
            rationale: "Approach B".to_string(),
        }).unwrap();

        engine.submit_response(session_id, agent4, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent5, InitialResponse::Accept).unwrap();

        engine.resolve_counter_proposals(session_id).unwrap();

        let session = engine.get_session(session_id).unwrap();
        match &session.active_amendment {
            PlanAmendment::ModifyGoal { new_goal } => {
                let g = new_goal.as_str();
                assert!(
                    g == "CounterA" || g == "CounterB",
                    "Expected one of the counter-proposals, got: {}",
                    g
                );
            }
            _ => panic!("Expected ModifyGoal"),
        }
    }

    #[test]
    fn test_backpressure_max_sessions() {
        let config = NegotiationConfig {
            max_active_sessions: 2,
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session1 = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal1") },
            "Rationale".to_string(),
        ).unwrap();

        let session2 = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal2") },
            "Rationale".to_string(),
        ).unwrap();

        let result = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal3") },
            "Rationale".to_string(),
        );

        assert!(matches!(result, Err(NegotiationError::Backpressure(2, 2))));

        engine.complete_session(session1).unwrap();
        engine.cleanup_completed();

        let session3 = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, vec![test_agent_id(2)],
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal3") },
            "Rationale".to_string(),
        ).unwrap();

        assert_eq!(engine.session_count(), 2);
    }

    #[test]
    fn test_rate_limit_per_agent() {
        let config = NegotiationConfig {
            rate_limit_per_agent: 2,
            rate_limit_window_secs: 60,
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal1") },
            "Rationale".to_string(),
        ).unwrap();

        engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal2") },
            "Rationale".to_string(),
        ).unwrap();

        let result = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal3") },
            "Rationale".to_string(),
        );

        assert!(matches!(result, Err(NegotiationError::RateLimited(_, 2))));
    }

    #[test]
    fn test_timeout_detection() {
        let config = NegotiationConfig {
            default_timeout: std::time::Duration::from_millis(10),
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal") },
            "Rationale".to_string(),
        ).unwrap();

        let timed_out = engine.check_timeout(session_id).unwrap();
        assert!(!timed_out);

        std::thread::sleep(std::time::Duration::from_millis(20));

        let timed_out = engine.check_timeout(session_id).unwrap();
        assert!(timed_out);
    }

    #[test]
    fn test_check_all_timeouts_aborts() {
        let config = NegotiationConfig {
            default_timeout: std::time::Duration::from_millis(10),
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal") },
            "Rationale".to_string(),
        ).unwrap();

        assert_eq!(engine.check_all_timeouts(), 0);

        std::thread::sleep(std::time::Duration::from_millis(20));

        assert_eq!(engine.check_all_timeouts(), 1);

        let session = engine.get_session(session_id).unwrap();
        assert!(matches!(session.phase, NegotiationPhase::Aborted(AbortReason::TotalTimeout)));
    }

    #[test]
    fn test_activity_updates_prevent_timeout() {
        let config = NegotiationConfig {
            default_timeout: std::time::Duration::from_millis(10),
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session_id = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal") },
            "Rationale".to_string(),
        ).unwrap();

        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_millis(8));
            engine.update_last_activity(session_id).unwrap();
        }

        let timed_out = engine.check_timeout(session_id).unwrap();
        assert!(!timed_out);
    }

    #[test]
    fn test_backpressure_load() {
        let config = NegotiationConfig {
            max_active_sessions: 10,
            ..NegotiationConfig::default()
        };
        let mut engine = NegotiationEngine::with_config(config);
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        assert_eq!(engine.backpressure_load(), 0.0);

        for i in 0..5 {
            engine.create_session(
                proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
                PlanAmendment::ModifyGoal { new_goal: GoalDescription::new(&format!("Goal{}", i)) },
                "Rationale".to_string(),
            ).unwrap();
        }

        assert_eq!(engine.backpressure_load(), 0.5);
    }

    #[test]
    fn test_cleanup_completed() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let session1 = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal1") },
            "Rationale".to_string(),
        ).unwrap();

        let session2 = engine.create_session(
            proposer, SovereigntyLevel::Level3, 0.9, target, affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Goal2") },
            "Rationale".to_string(),
        ).unwrap();

        assert_eq!(engine.session_count(), 2);

        engine.abort_session(session1, AbortReason::ProposerTerminated).unwrap();
        assert_eq!(engine.cleanup_completed(), 1);
        assert_eq!(engine.session_count(), 1);
    }

    // ─── IPC Integration Tests ─────────────────────────────────────

    #[test]
    fn test_create_session_from_ipc() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2), test_agent_id(3)];

        let request = CreateProposalRequest {
            proposer,
            target_agent: target,
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal from IPC"),
            },
            rationale: "Business requirement changed".to_string(),
            affected_agents: affected.clone(),
            consensus_threshold: 0.6,
        };

        let (session_id, broadcast_msgs) = engine
            .create_session_from_ipc(request, SovereigntyLevel::Level3, 0.9)
            .unwrap();

        assert_eq!(engine.session_count(), 1);

        // Should generate broadcast messages for all affected agents (excluding proposer)
        assert_eq!(broadcast_msgs.len(), 2);

        for msg in &broadcast_msgs {
            match msg {
                NegotiationMessage::ProposalBroadcast {
                    session_id: sid,
                    proposer: p,
                    target_agent: t,
                    amendment,
                    rationale,
                    affected_agents: aff,
                } => {
                    assert_eq!(*sid, session_id);
                    assert_eq!(*p, proposer);
                    assert_eq!(*t, target);
                    match amendment {
                        PlanAmendment::ModifyGoal { new_goal } => {
                            assert_eq!(new_goal.as_str(), "New goal from IPC");
                        }
                        _ => panic!("Expected ModifyGoal"),
                    }
                    assert_eq!(rationale, "Business requirement changed");
                    assert_eq!(aff.len(), 3); // proposer + 2 affected
                }
                _ => panic!("Expected ProposalBroadcast"),
            }
        }
    }

    #[test]
    fn test_create_session_from_ipc_requires_level3() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let target = test_agent_id(2);
        let affected = vec![test_agent_id(2)];

        let request = CreateProposalRequest {
            proposer,
            target_agent: target,
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            rationale: "Test".to_string(),
            affected_agents: affected,
            consensus_threshold: 0.6,
        };

        let result = engine.create_session_from_ipc(request, SovereigntyLevel::Level2, 0.5);
        assert!(matches!(result, Err(NegotiationError::InsufficientLevel(_))));
    }

    #[test]
    fn test_handle_ipc_response_message() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let responder = test_agent_id(2);
        let affected = vec![responder];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Test") },
        );

        let msg = NegotiationMessage::ResponseMessage {
            session_id,
            responder,
            response: InitialResponsePayload::Accept,
        };

        let result = engine.handle_ipc_message(msg).unwrap();
        assert!(result.is_none()); // No response needed

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.initial_responses.len(), 1);
        assert!(matches!(
            session.initial_responses.get(&responder),
            Some(InitialResponse::Accept)
        ));
    }

    #[test]
    fn test_handle_ipc_response_with_counter_proposal() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let responder = test_agent_id(2);
        let affected = vec![responder];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Original") },
        );

        let msg = NegotiationMessage::ResponseMessage {
            session_id,
            responder,
            response: InitialResponsePayload::CounterProposal {
                amendment: PlanAmendment::ModifyGoal {
                    new_goal: GoalDescription::new("Better approach"),
                },
                rationale: "More efficient".to_string(),
            },
        };

        engine.handle_ipc_message(msg).unwrap();

        let session = engine.get_session(session_id).unwrap();
        match session.initial_responses.get(&responder) {
            Some(InitialResponse::CounterProposal { rationale, .. }) => {
                assert_eq!(rationale, "More efficient");
            }
            _ => panic!("Expected CounterProposal"),
        }
    }

    #[test]
    fn test_handle_ipc_vote_message() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.8);

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Test") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        let msg = NegotiationMessage::VoteMessage {
            session_id,
            voter,
            decision: VoteDecision::Accept,
            rationale: "Agreed".to_string(),
            weight: 0.75, // This will be ignored; engine computes weight
        };

        engine.handle_ipc_message(msg).unwrap();

        let session = engine.get_session(session_id).unwrap();
        assert_eq!(session.final_votes.len(), 1);

        let vote = session.final_votes.get(&voter).unwrap();
        assert_eq!(vote.decision, VoteDecision::Accept);
        assert_eq!(vote.rationale, "Agreed");
        // Engine computed weight: 0.6 * 0.8 + 0.4 * 0.0 = 0.48
        assert!((vote.weight - 0.48).abs() < 0.001);
    }

    #[test]
    fn test_broadcast_proposal() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let agent4 = test_agent_id(4);
        let affected = vec![agent2, agent3, agent4];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(5),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("New goal") },
        );

        let broadcasts = engine.broadcast_proposal(session_id);
        assert_eq!(broadcasts.len(), 3); // 3 affected agents (excluding proposer)

        // Verify each broadcast contains correct data
        for msg in &broadcasts {
            match msg {
                NegotiationMessage::ProposalBroadcast {
                    proposer: p,
                    affected_agents: aff,
                    ..
                } => {
                    assert_eq!(*p, proposer);
                    assert_eq!(aff.len(), 4); // proposer + 3 affected
                }
                _ => panic!("Expected ProposalBroadcast"),
            }
        }
    }

    #[test]
    fn test_broadcast_voting_started() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(4),
            affected.clone(),
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Test") },
        );

        engine.submit_response(session_id, agent2, InitialResponse::Accept).unwrap();
        engine.submit_response(session_id, agent3, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();

        let voting_started = engine.broadcast_voting_started(session_id);
        assert_eq!(voting_started.len(), 2); // 2 affected agents

        for msg in &voting_started {
            match msg {
                NegotiationMessage::VotingStarted {
                    session_id: sid,
                    final_amendment,
                } => {
                    assert_eq!(*sid, session_id);
                    match final_amendment {
                        PlanAmendment::ModifyGoal { new_goal } => {
                            assert_eq!(new_goal.as_str(), "Test");
                        }
                        _ => panic!("Expected ModifyGoal"),
                    }
                }
                _ => panic!("Expected VotingStarted"),
            }
        }
    }

    #[test]
    fn test_broadcast_result_accepted() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let voter = test_agent_id(2);
        let affected = vec![voter];

        engine.set_bilateral_trust(voter, proposer, 0.9);

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Test") },
        );

        engine.submit_response(session_id, voter, InitialResponse::Accept).unwrap();
        engine.resolve_counter_proposals(session_id).unwrap();
        engine.submit_vote(session_id, voter, VoteDecision::Accept, "Yes".to_string()).unwrap();
        engine.submit_vote(session_id, proposer, VoteDecision::Accept, "Proposer yes".to_string()).unwrap();

        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);

        let result_msg = engine.broadcast_result(session_id);
        assert!(result_msg.is_some());

        match result_msg.unwrap() {
            NegotiationMessage::NegotiationResult {
                session_id: sid,
                outcome,
                final_amendment,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(outcome, Outcome::Accepted);
                assert!(final_amendment.is_some());
            }
            _ => panic!("Expected NegotiationResult"),
        }
    }

    #[test]
    fn test_broadcast_abort() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let affected = vec![test_agent_id(2)];

        let session_id = create_test_session(
            &mut engine,
            proposer,
            SovereigntyLevel::Level3,
            test_agent_id(3),
            affected,
            PlanAmendment::ModifyGoal { new_goal: GoalDescription::new("Test") },
        );

        engine.abort_session(session_id, AbortReason::ProposerTerminated).unwrap();

        let abort_msg = engine.broadcast_abort(session_id);
        assert!(abort_msg.is_some());

        match abort_msg.unwrap() {
            NegotiationMessage::NegotiationAborted {
                session_id: sid,
                reason,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(reason, AbortReason::ProposerTerminated);
            }
            _ => panic!("Expected NegotiationAborted"),
        }
    }

    #[test]
    fn test_ipc_full_negotiation_flow() {
        let mut engine = NegotiationEngine::new();
        let proposer = test_agent_id(1);
        let agent2 = test_agent_id(2);
        let agent3 = test_agent_id(3);
        let affected = vec![agent2, agent3];

        engine.set_bilateral_trust(agent2, proposer, 0.8);
        engine.set_bilateral_trust(agent3, proposer, 0.8);

        // Phase 1: Create session from IPC request
        let request = CreateProposalRequest {
            proposer,
            target_agent: test_agent_id(4),
            amendment: PlanAmendment::ModifyGoal {
                new_goal: GoalDescription::new("New goal"),
            },
            rationale: "Business requirement change".to_string(),
            affected_agents: affected.clone(),
            consensus_threshold: 0.6,
        };

        let (session_id, broadcasts) = engine
            .create_session_from_ipc(request, SovereigntyLevel::Level3, 0.9)
            .unwrap();

        assert_eq!(broadcasts.len(), 2);

        // Phase 2: Agents respond via IPC
        engine
            .handle_ipc_message(NegotiationMessage::ResponseMessage {
                session_id,
                responder: agent2,
                response: InitialResponsePayload::Accept,
            })
            .unwrap();

        engine
            .handle_ipc_message(NegotiationMessage::ResponseMessage {
                session_id,
                responder: agent3,
                response: InitialResponsePayload::Accept,
            })
            .unwrap();

        // Phase 3: Resolve counter-proposals (none in this case)
        engine.resolve_counter_proposals(session_id).unwrap();

        // Phase 4: Broadcast voting started
        let voting_started = engine.broadcast_voting_started(session_id);
        assert_eq!(voting_started.len(), 2);

        // Phase 4: Agents vote via IPC
        engine
            .handle_ipc_message(NegotiationMessage::VoteMessage {
                session_id,
                voter: agent2,
                decision: VoteDecision::Accept,
                rationale: "Agreed".to_string(),
                weight: 0.0, // ignored
            })
            .unwrap();

        engine
            .handle_ipc_message(NegotiationMessage::VoteMessage {
                session_id,
                voter: agent3,
                decision: VoteDecision::Accept,
                rationale: "Agreed".to_string(),
                weight: 0.0, // ignored
            })
            .unwrap();

        engine
            .handle_ipc_message(NegotiationMessage::VoteMessage {
                session_id,
                voter: proposer,
                decision: VoteDecision::Accept,
                rationale: "Proposer agrees".to_string(),
                weight: 0.0, // ignored
            })
            .unwrap();

        // Phase 5: Complete session
        let outcome = engine.complete_session(session_id).unwrap();
        assert_eq!(outcome, Outcome::Accepted);

        // Broadcast result
        let result_msg = engine.broadcast_result(session_id);
        assert!(result_msg.is_some());

        match result_msg.unwrap() {
            NegotiationMessage::NegotiationResult {
                outcome,
                final_amendment: Some(PlanAmendment::ModifyGoal { new_goal }),
                ..
            } => {
                assert_eq!(outcome, Outcome::Accepted);
                assert_eq!(new_goal.as_str(), "New goal");
            }
            _ => panic!("Expected NegotiationResult"),
        }

        // Verify trust updates
        let trust2 = engine.get_bilateral_trust(agent2, proposer);
        assert!((trust2 - 0.85).abs() < 0.001); // +0.05

        let trust3 = engine.get_bilateral_trust(agent3, proposer);
        assert!((trust3 - 0.85).abs() < 0.001); // +0.05
    }
}

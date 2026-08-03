//! Lightweight AFS integration for negotiation module.
//!
//! Stores proposals in `/shared/proposals/` for cross-agent visibility.
//! Bilateral trust snapshots are exposed via `/shared/bilateral_trust/`.
//!
//! This is a lightweight integration — it uses the existing AFS shared
//! directory without modifying core AFS architecture.

use crate::AgentId;
use crate::fs::{AgentFileSystem, OpenMode, FsError};
use crate::sovereignty::SovereigntyLevel;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

use super::{NegotiationSession, PlanAmendment, SessionId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalFile {
    pub session_id: SessionId,
    pub proposer: AgentId,
    pub target_agent: AgentId,
    pub affected_agents: Vec<AgentId>,
    pub amendment: PlanAmendment,
    pub rationale: String,
    pub status: String,
    pub created_at: String,
}

impl From<&NegotiationSession> for ProposalFile {
    fn from(session: &NegotiationSession) -> Self {
        ProposalFile {
            session_id: session.id,
            proposer: session.proposal.proposer,
            target_agent: session.proposal.target_agent,
            affected_agents: session.affected_agents.clone(),
            amendment: session.active_amendment.clone(),
            rationale: session.proposal.rationale.clone(),
            status: format!("{:?}", session.phase),
            created_at: format!("{:?}", session.created_at),
        }
    }
}

/// Write a proposal to `/shared/proposals/<session_id>.json`.
pub fn write_proposal_to_afs(
    afs: &AgentFileSystem,
    session: &NegotiationSession,
    caller: AgentId,
    caller_level: SovereigntyLevel,
) -> Result<(), FsError> {
    let proposal_file = ProposalFile::from(session);
    let json = serde_json::to_string_pretty(&proposal_file)
        .map_err(|_| FsError::InvalidPath)?;

    let path = format!("/shared/proposals/{}.json", session.id);
    let mut handle = afs.open(&path, OpenMode::Write, caller, caller_level)?;
    afs.write(&mut handle, json.as_bytes())?;
    Ok(())
}

/// List all proposals in `/shared/proposals/`.
pub fn list_proposals_in_afs(
    afs: &AgentFileSystem,
    caller: AgentId,
    caller_level: SovereigntyLevel,
) -> Result<Vec<String>, FsError> {
    afs.list("/shared/proposals", caller, caller_level)
}

/// Read a specific proposal from `/shared/proposals/<session_id>.json`.
pub fn read_proposal_from_afs(
    afs: &AgentFileSystem,
    session_id: SessionId,
    caller: AgentId,
    caller_level: SovereigntyLevel,
) -> Result<ProposalFile, FsError> {
    let path = format!("/shared/proposals/{}.json", session_id);
    let mut handle = afs.open(&path, OpenMode::Read, caller, caller_level)?;
    let mut buf = vec![0u8; 65536];
    let n = afs.read(&mut handle, &mut buf)?;
    let json = std::str::from_utf8(&buf[..n]).map_err(|_| FsError::InvalidPath)?;
    serde_json::from_str(json).map_err(|_| FsError::InvalidPath)
}

/// Write bilateral trust snapshot to `/shared/bilateral_trust/<from_id>_to_<to_id>.txt`.
pub fn write_bilateral_trust_to_afs(
    afs: &AgentFileSystem,
    from: AgentId,
    to: AgentId,
    trust_score: f64,
    caller: AgentId,
    caller_level: SovereigntyLevel,
) -> Result<(), FsError> {
    let path = format!("/shared/bilateral_trust/{}_to_{}.txt", from, to);
    let content = format!("{:.4}", trust_score);
    let mut handle = afs.open(&path, OpenMode::Write, caller, caller_level)?;
    afs.write(&mut handle, content.as_bytes())?;
    Ok(())
}

/// Write all bilateral trust entries to AFS.
pub fn write_all_bilateral_trust_to_afs(
    afs: &AgentFileSystem,
    trust_map: &HashMap<(AgentId, AgentId), f64>,
    caller: AgentId,
    caller_level: SovereigntyLevel,
) -> Result<(), FsError> {
    for ((from, to), score) in trust_map {
        write_bilateral_trust_to_afs(afs, *from, *to, *score, caller, caller_level)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentRecord;
    use crate::sovereignty::SovereigntyLevel;
    use std::sync::{Arc, RwLock};

    fn test_agent_id(n: u8) -> AgentId {
        let mut bytes = [0u8; 16];
        bytes[0] = n;
        AgentId::from_bytes(bytes)
    }

    fn setup_afs() -> (AgentFileSystem, AgentId, AgentId) {
        let records = Arc::new(RwLock::new(HashMap::new()));
        let afs = AgentFileSystem::new(records.clone());
        let caller = test_agent_id(1);
        let model = crate::program::ModelDescriptor::new("test", "test-model");
        let prompt = crate::program::PromptDescriptor::new("test prompt");
        let record = AgentRecord::new(
            caller,
            crate::Intent::new_root(crate::IntentId::new(), "test", Some(caller)),
            crate::AgentProgram::new(model, prompt),
        );
        records.write().unwrap().insert(caller, record);
        (afs, caller, test_agent_id(2))
    }

    #[test]
    fn test_write_and_read_bilateral_trust() {
        let (afs, caller, other) = setup_afs();
        let level = SovereigntyLevel::Level2;

        write_bilateral_trust_to_afs(&afs, other, caller, 0.75, caller, level).unwrap();

        let path = format!("/shared/bilateral_trust/{}_to_{}.txt", other, caller);
        let mut handle = afs.open(&path, OpenMode::Read, caller, level).unwrap();
        let mut buf = vec![0u8; 100];
        let n = afs.read(&mut handle, &mut buf).unwrap();
        let content = std::str::from_utf8(&buf[..n]).unwrap();
        assert_eq!(content, "0.7500");
    }
}

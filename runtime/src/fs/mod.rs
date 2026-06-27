//! Agent File System (AFS) — RFC-004
//!
//! Procfs-inspired virtual filesystem for agent knowledge storage.
//! - `/proc/<id>/*`: Virtual (read-only, generated from AgentRecord)
//! - `/home/<id>/*`: Private per-agent storage
//! - `/shared/*`: Shared cross-agent storage

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::{AgentId, AgentRecord, SovereigntyLevel};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ========== Path Types ==========

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FsRoot { Proc, Home, Shared }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsPath {
    pub root: FsRoot,
    pub agent_id: Option<AgentId>,
    pub segments: Vec<String>,
}

impl FsPath {
    pub fn parse(path: &str) -> Result<Self, FsError> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        if parts.is_empty() { return Err(FsError::InvalidPath); }

        let root = match parts[0] {
            "proc" => FsRoot::Proc,
            "home" => FsRoot::Home,
            "shared" => FsRoot::Shared,
            _ => return Err(FsError::InvalidPath),
        };

        let (agent_id, segments) = match root {
            FsRoot::Proc | FsRoot::Home => {
                if parts.len() < 2 { return Err(FsError::InvalidPath); }
                let id_str = parts[1];
                let uuid = uuid::Uuid::parse_str(id_str).map_err(|_| FsError::InvalidPath)?;
                let id = AgentId::from_bytes(uuid.into_bytes());
                (Some(id), parts[2..].iter().map(|s| s.to_string()).collect())
            }
            FsRoot::Shared => (None, parts[1..].iter().map(|s| s.to_string()).collect()),
        };

        Ok(FsPath { root, agent_id, segments })
    }

    pub fn to_string(&self) -> String {
        let root = match self.root {
            FsRoot::Proc => "proc",
            FsRoot::Home => "home",
            FsRoot::Shared => "shared",
        };
        let agent = self.agent_id.map(|id| format!("/{}", id)).unwrap_or_default();
        let segments = if self.segments.is_empty() { String::new() }
                       else { format!("/{}", self.segments.join("/")) };
        format!("/{}{}{}", root, agent, segments)
    }
}

// ========== File Content ==========

#[derive(Debug, Clone)]
pub enum FileContent {
    Virtual(fn(&AgentRecord) -> String),
    Stored(Vec<u8>),
}

// ========== File Metadata ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub owner: AgentId,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub size: usize,
    pub version: u64,
}

// ========== File Lock ==========

#[derive(Debug)]
pub struct FileLockState {
    pub readers: u32,
    pub writer: Option<AgentId>,
}

impl Default for FileLockState {
    fn default() -> Self { Self { readers: 0, writer: None } }
}

// ========== File Entry ==========

#[derive(Debug)]
pub struct FileEntry {
    pub content: FileContent,
    pub metadata: FileMetadata,
    pub lock: Arc<RwLock<FileLockState>>,
}

// ========== File Handle ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode { Read, Write, ReadWrite, Append }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whence { Set, Current, End }

#[derive(Debug)]
pub struct FileHandle {
    pub id: u64,
    pub path: FsPath,
    pub mode: OpenMode,
    pub position: usize,
    pub owner: AgentId,
}

// ========== Transaction ==========

#[derive(Debug)]
pub struct Transaction {
    pub id: u64,
    pub caller: AgentId,
    pub operations: Vec<PendingOperation>,
}

#[derive(Debug, Clone)]
pub enum PendingOperation {
    Write { path: String, content: Vec<u8> },
    Delete { path: String },
    Export { from: String, to: String },
}

// ========== Errors ==========

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FsError {
    #[error("path not found")]
    NotFound,
    #[error("permission denied: required level {required}, have {current}")]
    PermissionDenied { required: u8, current: u8 },
    #[error("not a file")]
    NotAFile,
    #[error("not a directory")]
    NotADirectory,
    #[error("already exists")]
    AlreadyExists,
    #[error("invalid path")]
    InvalidPath,
    #[error("file locked")]
    FileLocked,
    #[error("transaction aborted")]
    TransactionAborted,
    #[error("sensitive operation denied: {operation}")]
    SensitiveOpDenied { operation: String },
}

// ========== Agent File System ==========

#[derive(Debug)]
pub struct AgentFileSystem {
    private: RwLock<HashMap<AgentId, HashMap<String, FileEntry>>>,
    shared: RwLock<HashMap<String, FileEntry>>,
    proc_readers: HashMap<String, fn(&AgentRecord) -> String>,
    records: Arc<RwLock<HashMap<AgentId, AgentRecord>>>,
    next_handle_id: RwLock<u64>,
}

impl AgentFileSystem {
    pub fn new(records: Arc<RwLock<HashMap<AgentId, AgentRecord>>>) -> Self {
        let mut proc_readers: HashMap<String, fn(&AgentRecord) -> String> = HashMap::new();
        proc_readers.insert("trust_score".to_string(), |r: &AgentRecord| {
            format!("{:.4}", r.scheduling_context.priority)
        });
        proc_readers.insert("state".to_string(), |r: &AgentRecord| {
            r.state.current.clone()
        });

        Self {
            private: RwLock::new(HashMap::new()),
            shared: RwLock::new(HashMap::new()),
            proc_readers,
            records,
            next_handle_id: RwLock::new(1),
        }
    }

    fn next_id(&self) -> u64 {
        let mut id = self.next_handle_id.write().unwrap();
        let current = *id;
        *id += 1;
        current
    }

    // ========== POSIX API ==========

    pub fn open(
        &self,
        path: &str,
        mode: OpenMode,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<FileHandle, FsError> {
        let fs_path = FsPath::parse(path)?;

        // Permission check
        self.check_permission(&fs_path, mode, caller, trust_level)?;

        // Acquire lock
        match fs_path.root {
            FsRoot::Proc => {} // No lock needed for virtual files
            FsRoot::Home => {
                let private = self.private.read().unwrap();
                if let Some(agent_files) = private.get(&fs_path.agent_id.unwrap()) {
                    let key = fs_path.segments.join("/");
                    if let Some(entry) = agent_files.get(&key) {
                        let mut lock = entry.lock.write().unwrap();
                        match mode {
                            OpenMode::Read => lock.readers += 1,
                            OpenMode::Write | OpenMode::Append => {
                                if lock.writer.is_some() { return Err(FsError::FileLocked); }
                                lock.writer = Some(caller);
                            }
                            OpenMode::ReadWrite => {
                                if lock.writer.is_some() { return Err(FsError::FileLocked); }
                                lock.readers += 1;
                                lock.writer = Some(caller);
                            }
                        }
                    }
                }
            }
            FsRoot::Shared => {
                let shared = self.shared.read().unwrap();
                let key = fs_path.segments.join("/");
                if let Some(entry) = shared.get(&key) {
                    let mut lock = entry.lock.write().unwrap();
                    match mode {
                        OpenMode::Read => lock.readers += 1,
                        OpenMode::Write | OpenMode::Append => {
                            if lock.writer.is_some() { return Err(FsError::FileLocked); }
                            lock.writer = Some(caller);
                        }
                        OpenMode::ReadWrite => {
                            if lock.writer.is_some() { return Err(FsError::FileLocked); }
                            lock.readers += 1;
                            lock.writer = Some(caller);
                        }
                    }
                }
            }
        }

        Ok(FileHandle {
            id: self.next_id(),
            path: fs_path,
            mode,
            position: 0,
            owner: caller,
        })
    }

    pub fn read(&self, handle: &mut FileHandle, buf: &mut [u8]) -> Result<usize, FsError> {
        let content = match handle.path.root {
            FsRoot::Proc => {
                let field = handle.path.segments.first().ok_or(FsError::NotFound)?;
                let reader = self.proc_readers.get(field).ok_or(FsError::NotFound)?;
                let records = self.records.read().unwrap();
                let record = records.get(&handle.path.agent_id.unwrap()).ok_or(FsError::NotFound)?;
                reader(record).into_bytes()
            }
            FsRoot::Home => {
                let private = self.private.read().unwrap();
                let agent_files = private.get(&handle.path.agent_id.unwrap()).ok_or(FsError::NotFound)?;
                let key = handle.path.segments.join("/");
                let entry = agent_files.get(&key).ok_or(FsError::NotFound)?;
                match &entry.content {
                    FileContent::Stored(data) => data.clone(),
                    _ => return Err(FsError::NotAFile),
                }
            }
            FsRoot::Shared => {
                let shared = self.shared.read().unwrap();
                let key = handle.path.segments.join("/");
                let entry = shared.get(&key).ok_or(FsError::NotFound)?;
                match &entry.content {
                    FileContent::Stored(data) => data.clone(),
                    _ => return Err(FsError::NotAFile),
                }
            }
        };

        let available = content.len().saturating_sub(handle.position);
        let to_read = buf.len().min(available);
        buf[..to_read].copy_from_slice(&content[handle.position..handle.position + to_read]);
        handle.position += to_read;
        Ok(to_read)
    }

    pub fn write(&self, handle: &mut FileHandle, buf: &[u8]) -> Result<usize, FsError> {
        match handle.path.root {
            FsRoot::Proc => return Err(FsError::PermissionDenied { required: 0, current: 0 }),
            FsRoot::Home => {
                let mut private = self.private.write().unwrap();
                let agent_files = private.entry(handle.path.agent_id.unwrap()).or_insert_with(HashMap::new);
                let key = handle.path.segments.join("/");

                let entry = agent_files.entry(key).or_insert_with(|| FileEntry {
                    content: FileContent::Stored(Vec::new()),
                    metadata: FileMetadata {
                        owner: handle.owner,
                        created_at: Utc::now(),
                        modified_at: Utc::now(),
                        size: 0,
                        version: 0,
                    },
                    lock: Arc::new(RwLock::new(FileLockState::default())),
                });

                if let FileContent::Stored(ref mut data) = entry.content {
                    match handle.mode {
                        OpenMode::Append => data.extend_from_slice(buf),
                        _ => {
                            if handle.position == 0 {
                                *data = buf.to_vec();
                            } else {
                                data.resize(handle.position, 0);
                                data.extend_from_slice(buf);
                            }
                        }
                    }
                    handle.position = data.len();
                    entry.metadata.size = data.len();
                    entry.metadata.modified_at = Utc::now();
                    entry.metadata.version += 1;
                }
                Ok(buf.len())
            }
            FsRoot::Shared => {
                let mut shared = self.shared.write().unwrap();
                let key = handle.path.segments.join("/");

                let entry = shared.entry(key).or_insert_with(|| FileEntry {
                    content: FileContent::Stored(Vec::new()),
                    metadata: FileMetadata {
                        owner: handle.owner,
                        created_at: Utc::now(),
                        modified_at: Utc::now(),
                        size: 0,
                        version: 0,
                    },
                    lock: Arc::new(RwLock::new(FileLockState::default())),
                });

                if let FileContent::Stored(ref mut data) = entry.content {
                    match handle.mode {
                        OpenMode::Append => data.extend_from_slice(buf),
                        _ => {
                            if handle.position == 0 {
                                *data = buf.to_vec();
                            } else {
                                data.resize(handle.position, 0);
                                data.extend_from_slice(buf);
                            }
                        }
                    }
                    handle.position = data.len();
                    entry.metadata.size = data.len();
                    entry.metadata.modified_at = Utc::now();
                    entry.metadata.version += 1;
                }
                Ok(buf.len())
            }
        }
    }

    pub fn lseek(&self, handle: &mut FileHandle, offset: isize, whence: Whence) -> Result<usize, FsError> {
        let new_pos = match whence {
            Whence::Set => offset as usize,
            Whence::Current => (handle.position as isize + offset) as usize,
            Whence::End => {
                // Would need file size, for now just return current
                handle.position
            }
        };
        handle.position = new_pos;
        Ok(new_pos)
    }

    pub fn close(&self, handle: FileHandle) -> Result<(), FsError> {
        // Release lock
        match handle.path.root {
            FsRoot::Proc => {} // No lock
            FsRoot::Home => {
                let private = self.private.read().unwrap();
                if let Some(agent_files) = private.get(&handle.path.agent_id.unwrap()) {
                    let key = handle.path.segments.join("/");
                    if let Some(entry) = agent_files.get(&key) {
                        let mut lock = entry.lock.write().unwrap();
                        match handle.mode {
                            OpenMode::Read => lock.readers = lock.readers.saturating_sub(1),
                            OpenMode::Write | OpenMode::Append => lock.writer = None,
                            OpenMode::ReadWrite => {
                                lock.readers = lock.readers.saturating_sub(1);
                                lock.writer = None;
                            }
                        }
                    }
                }
            }
            FsRoot::Shared => {
                let shared = self.shared.read().unwrap();
                let key = handle.path.segments.join("/");
                if let Some(entry) = shared.get(&key) {
                    let mut lock = entry.lock.write().unwrap();
                    match handle.mode {
                        OpenMode::Read => lock.readers = lock.readers.saturating_sub(1),
                        OpenMode::Write | OpenMode::Append => lock.writer = None,
                        OpenMode::ReadWrite => {
                            lock.readers = lock.readers.saturating_sub(1);
                            lock.writer = None;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn list(&self, path: &str, caller: AgentId, trust_level: SovereigntyLevel) -> Result<Vec<String>, FsError> {
        let fs_path = FsPath::parse(path)?;
        self.check_list_permission(&fs_path, caller, trust_level)?;

        match fs_path.root {
            FsRoot::Proc => {
                if let Some(agent_id) = fs_path.agent_id {
                    let records = self.records.read().unwrap();
                    if records.contains_key(&agent_id) {
                        return Ok(self.proc_readers.keys().cloned().collect());
                    }
                }
                Err(FsError::NotFound)
            }
            FsRoot::Home => {
                let private = self.private.read().unwrap();
                if let Some(agent_files) = private.get(&fs_path.agent_id.unwrap()) {
                    return Ok(agent_files.keys().cloned().collect());
                }
                Err(FsError::NotFound)
            }
            FsRoot::Shared => {
                let shared = self.shared.read().unwrap();
                Ok(shared.keys().cloned().collect())
            }
        }
    }

    // ========== Permission Checks ==========

    fn check_permission(
        &self,
        path: &FsPath,
        mode: OpenMode,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<(), FsError> {
        let level = trust_level.as_u8();
        let is_owner = path.agent_id == Some(caller);

        match path.root {
            FsRoot::Proc => {
                if mode != OpenMode::Read {
                    return Err(FsError::PermissionDenied { required: 0, current: level });
                }
                if !is_owner && level < 1 {
                    return Err(FsError::PermissionDenied { required: 1, current: level });
                }
            }
            FsRoot::Home => {
                if !is_owner {
                    return Err(FsError::PermissionDenied { required: 0, current: level });
                }
                // Owner can always read/write own home
            }
            FsRoot::Shared => {
                if level < 2 {
                    return Err(FsError::PermissionDenied { required: 2, current: level });
                }
            }
        }
        Ok(())
    }

    fn check_list_permission(
        &self,
        path: &FsPath,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<(), FsError> {
        let level = trust_level.as_u8();
        let is_owner = path.agent_id == Some(caller);

        match path.root {
            FsRoot::Proc => {
                if !is_owner && level < 1 {
                    return Err(FsError::PermissionDenied { required: 1, current: level });
                }
            }
            FsRoot::Home => {
                if !is_owner {
                    return Err(FsError::PermissionDenied { required: 0, current: level });
                }
            }
            FsRoot::Shared => {
                if level < 2 {
                    return Err(FsError::PermissionDenied { required: 2, current: level });
                }
            }
        }
        Ok(())
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
    fn test_path_parse_proc() {
        let path = FsPath::parse("/proc/01000000-0000-0000-0000-000000000000/trust_score").unwrap();
        assert_eq!(path.root, FsRoot::Proc);
        assert!(path.agent_id.is_some());
        assert_eq!(path.segments, vec!["trust_score"]);
    }

    #[test]
    fn test_path_parse_home() {
        let path = FsPath::parse("/home/01000000-0000-0000-0000-000000000000/memory.json").unwrap();
        assert_eq!(path.root, FsRoot::Home);
        assert!(path.agent_id.is_some());
        assert_eq!(path.segments, vec!["memory.json"]);
    }

    #[test]
    fn test_path_parse_shared() {
        let path = FsPath::parse("/shared/report.md").unwrap();
        assert_eq!(path.root, FsRoot::Shared);
        assert!(path.agent_id.is_none());
        assert_eq!(path.segments, vec!["report.md"]);
    }

    #[test]
    fn test_path_invalid() {
        assert!(FsPath::parse("/invalid/path").is_err());
    }

    // More tests would go here...
}

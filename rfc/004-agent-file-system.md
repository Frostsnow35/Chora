# RFC-004 — Agent File System (AFS)

| Field | Value |
|---|---|
| Status | Draft |
| Author | — |
| Created | 2026-06-27 |
| Depends on | `rfc/001-agent-process.md`, `rfc/003-sovereignty-system.md` |

---

## Abstract

This RFC proposes the **Agent File System (AFS)** — a **procfs-inspired virtual filesystem** that provides each agent with persistent, structured, permission-gated knowledge storage.

AFS treats agent knowledge as files in a virtual filesystem, mirroring how Linux's `/proc` exposes process state as files. It replaces the existing `MemoryTool` (flat KV store) with a richer, OS-semantic system that:

1. **Exposes agent state as virtual files** (`/proc/<agent_id>/trust_score`) — O(1) read, zero storage
2. **Provides private per-agent storage** (`/home/<agent_id>/...`) — the agent's "working memory"
3. **Provides shared cross-agent storage** (`/shared/...`) — collaboration space
4. **Gate-keeps access via TrustLevel** — Level 0 → Level 3 progressively unlocks capabilities

AFS is a **virtual filesystem**, not a disk filesystem. Files are backed by in-memory structures (with optional persistence hooks), and virtual files (`/proc`) are computed on-demand from live agent state.

---

## 1. Motivation

### 1.1 Limitations of Current MemoryTool

The existing `MemoryTool` (RFC-001 §8) is a flat `HashMap<String, Value>` behind a `Mutex`:
- No hierarchy (all keys in one flat namespace)
- No access control (any agent can read/write anything)
- No visibility into agent state (cannot introspect another agent's trust score, intent, etc.)
- No cross-agent knowledge sharing (each `MemoryTool` is a separate, isolated instance)
- No OS semantics (just a KV store, not a "filesystem")

### 1.2 Why a Filesystem?

The OS metaphor is central to Chora. Every major OS component has an agent analogue:
- **Process** → Agent (RFC-001)
- **Scheduler** → Runtime + Scheduler (RFC-002)
- **Capability system** → Sovereignty (RFC-003)
- **IPC** → UnidirectionalChannel, BroadcastChannel
- **File system** → ??? (missing — this RFC)

A filesystem is the natural abstraction for **persistent, structured knowledge**. It provides:
- **Hierarchical organization** (`/home/<id>/docs/report.md`)
- **Permission semantics** (read/write/execute by owner/group/others)
- **Introspection** (`/proc` exposes live process state)
- **Shared resources** (`/dev`, `/tmp`, shared directories)
- **Familiar API** (open/read/write/close — well-understood by developers)

### 1.3 Why Procfs-style?

Linux's `/proc` is a **virtual filesystem**: files don't exist on disk; they're computed on-demand from kernel data structures. This is the perfect model for AFS:

- **Zero storage overhead** for introspection files (trust score, intent, state)
- **Always fresh** (reads always reflect current agent state, no stale copies)
- **O(1) read** (no file lookup, just a function call)
- **Read-only by design** (users can't corrupt kernel state by writing to /proc)

The `/home` and `/shared` parts of AFS are backed by real storage (HashMaps), but `/proc` is purely virtual.

---

## 2. Path Structure

```
/
├── proc/                          # Virtual (read-only, generated on-demand)
│   └── <agent_id>/
│       ├── trust_score            # Current trust score (0.0-1.0)
│       ├── sovereignty_level      # Current sovereignty level (0-3)
│       ├── state                  # Current scheduling state
│       ├── intent                 # Current intent (goal, constraints)
│       ├── constitution           # Immutable constitution (boundaries, purpose)
│       ├── reasoning_config       # Current LLM reasoning parameters
│       └── metrics                # Steps executed, tokens consumed, etc.
│
├── home/                          # Private (per-agent storage, HashMap-backed)
│   └── <agent_id>/
│       └── (arbitrary files/dirs)
│
└── shared/                        # Shared (cross-agent storage)
    └── (arbitrary files/dirs)
```

### Path Semantics

| Path | Type | Backing | Write Access | Read Access |
|------|------|---------|--------------|-------------|
| `/proc/<agent_id>/*` | Virtual | Generated from AgentRecord | Never | By TrustLevel |
| `/home/<agent_id>/*` | Private | HashMap per agent | Owner only (L0+) | By TrustLevel |
| `/shared/*` | Shared | Global HashMap | L2+ | L2+ |

---

## 3. Permission Model

Permissions are tied to **sovereignty level** (derived from Trust Score):

### 3.1 Base Permissions (by TrustLevel)

| TrustLevel | `/proc` (self) | `/proc` (others) | `/home` (self) | `/home` (others) | `/shared` |
|------------|----------------|------------------|----------------|------------------|-----------|
| **Level 0** (0.0-0.59) | Read | ✗ | Read/Write | ✗ | ✗ |
| **Level 1** (0.6-0.74) | Read | **Read** | Read/Write | ✗ | ✗ |
| **Level 2** (0.75-0.89) | Read | Read | Read/Write | **Read** | **Read/Write** |
| **Level 3** (0.9-1.0) | Read | Read | Read/Write | Read | **Full** (can create subdirs) |

**Key design decisions**:
- **Level 0 → Level 1**: Unlocks ability to *observe* other agents (read their /proc). This aligns with "reject boundary-violating requests" — you need to see what others are doing to judge.
- **Level 1 → Level 2**: Unlocks *collaboration* (read other's /home, access /shared). This aligns with "self-terminate" — agent can independently coordinate with peers.
- **Level 2 → Level 3**: Unlocks *administration* (create shared directories, manage shared structure). This aligns with "propose intent amendments" — agent can reshape the shared environment.

### 3.2 Fine-grained Restrictions on Sensitive Operations

Beyond TrustLevel, certain operations have **additional constraints**:

| Operation | Required TrustLevel | Additional Restriction |
|-----------|---------------------|------------------------|
| `delete(/shared/...)` | L3 | Must be **owner** OR **admin** |
| `write(/shared/...)` (not owner) | L2 | Must be **owner** OR **listed collaborator** |
| `export(/home/... → /shared/...)` | L2 | None |
| `delete(/home/self/...)` | L0 | Must be **owner** |
| `mkdir(/shared/...)` | L3 | None |

### 3.3 Collaborator Lists (for shared files)

Shared files can have an explicit **collaborator list**:
```rust
pub struct SharedFileMetadata {
    pub base: FileMetadata,
    pub collaborators: Vec<AgentId>,  // Explicit write access
    pub is_public: bool,              // If true, any L2+ can write
}
```

- If `is_public = true`: Any L2+ agent can modify the file
- If `is_public = false`: Only `owner` or agents in `collaborators` can modify
- This enables controlled collaboration without opening files to everyone

### 3.4 Export Operation

Level 2+ agents can **export** a file from their private space to shared:
```
export("/home/A/report.md", "/shared/reports/agent_a_report.md")
```
- Copies content (not a hardlink — changes to original don't affect exported copy)
- Sets owner metadata (so other agents know who created it)
- Default: `is_public = false` (only collaborators can modify)
- Requires L2+ (collaboration permission)

---

## 4. Performance Design

### 4.1 O(1) Operations

| Operation | Complexity | Mechanism |
|-----------|------------|-----------|
| `open(/proc/<id>/<field>)` | **O(1)** | Direct struct field access, no lookup |
| `open(/home/<id>/<path>)` | **O(1)** | HashMap lookup by `(agent_id, path)` |
| `read(handle)` | **O(content_read)** | Sequential read from buffer |
| `write(handle)` | **O(content_written)** | Sequential write to buffer |
| `lseek(handle)` | **O(1)** | Update position field |
| `close(handle)` | **O(1)** | Release lock, drop handle |
| `open(/shared/<path>)` | **O(1)** | HashMap lookup by `path` |
| `delete(path)` | **O(1)** | HashMap remove |
| `export(...)` | **O(content_size)** | Copy content, update metadata |
| `list(<dir>)` | **O(k)** | k = entries in directory (use prefix index) |
| `commit_transaction` | **O(ops × lock_acquire)** | Acquire all locks, apply ops, release |

### 4.2 Lock Strategy

| File Type | Lock Type | Concurrent Reads | Concurrent Writes |
|-----------|-----------|------------------|-------------------|
| `/proc/*` | **None** (virtual) | ✅ Unlimited | ❌ N/A |
| `/home/*` | **RwLock** per file | ✅ Unlimited | ❌ Exclusive |
| `/shared/*` | **RwLock** per file | ✅ Unlimited | ❌ Exclusive |

- **`open(mode=Read)`**: Acquires shared lock (allows concurrent readers)
- **`open(mode=Write)`**: Acquires exclusive lock (blocks other writers/readers)
- **`close()`**: Releases lock
- **Transaction**: Acquires all locks upfront (two-phase locking to avoid deadlock)

### 4.3 Transaction Atomicity

Transactions use **two-phase locking (2PL)** for deadlock-free atomicity:
1. **Growing phase**: Acquire all locks needed by staged operations
2. **Shrinking phase**: Apply all operations, release all locks

If any lock acquisition fails → **abort and rollback**.
If all locks acquired → **apply atomically, then release**.

### 4.4 Zero Heap Allocation on Read Hot Path

- `/proc` reads return borrowed `&str` from pre-allocated buffers (no String allocation)
- Path parsing uses string interning (path segments cached in a global intern table)
- FileEntry metadata is fixed-size struct (no dynamic fields)

### 4.5 Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| `/proc` open+read | **< 200ns** | Struct field access + copy |
| `/home` open | **< 500ns** | HashMap lookup + lock acquire |
| `/home` read/write | **< 100ns per KB** | Sequential buffer access |
| Transaction commit (3 ops) | **< 5μs** | 3 lock acquires + 3 ops |
| Concurrent read throughput | **> 10M reads/sec** | RwLock uncontended reads |
| Memory overhead | **~300 bytes per file entry** | FileEntry + lock state |

These numbers should be validated via benchmarks in the experiment.

---

## 5. Core Types

```rust
/// A parsed filesystem path.
pub struct FsPath {
    pub root: FsRoot,              // Proc, Home, Shared
    pub agent_id: Option<AgentId>, // For /proc/<id> and /home/<id>
    pub segments: Vec<String>,     // Remaining path segments
}

pub enum FsRoot {
    Proc,
    Home,
    Shared,
}

/// A file entry in the filesystem.
pub struct FileEntry {
    pub content: FileContent,
    pub metadata: FileMetadata,
    pub lock: Arc<RwLock<FileLockState>>,  // NEW: per-file lock
}

pub enum FileContent {
    /// Computed on-demand (for /proc)
    Virtual(VirtualReader),
    /// Stored (for /home, /shared)
    Stored(Vec<u8>),
}

pub struct FileMetadata {
    pub owner: AgentId,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub size: usize,
    pub version: u64,  // NEW: for optimistic locking
}

/// Function that generates virtual file content on demand.
pub type VirtualReader = fn(&AgentRecord) -> String;

/// Filesystem errors.
pub enum FsError {
    NotFound,
    PermissionDenied { required: u8, current: u8 },
    NotAFile,
    NotADirectory,
    AlreadyExists,
    InvalidPath,
    FileLocked,           // NEW: conflict detection
    TransactionAborted,   // NEW: transaction conflict
    SensitiveOpDenied { operation: String },  // NEW: fine-grained restriction
}

/// POSIX-style file handle (like Unix file descriptor)
pub struct FileHandle {
    pub id: u64,
    pub path: FsPath,
    pub mode: OpenMode,
    pub position: usize,
    pub owner: AgentId,
    pub lock_guard: Option<FileLockGuard>,  // NEW: holds lock while open
}

pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
    Append,
}

pub enum Whence {
    Set,      // SEEK_SET
    Current,  // SEEK_CUR
    End,      // SEEK_END
}

/// Transaction for atomic multi-file operations
pub struct Transaction {
    pub id: u64,
    pub caller: AgentId,
    pub operations: Vec<PendingOperation>,
    pub locks: Vec<FileLockGuard>,
}

pub enum PendingOperation {
    Write { path: String, content: Vec<u8> },
    Delete { path: String },
    Export { from: String, to: String },
}
```

---

## 6. API

### 6.1 POSIX-style File Operations

```rust
impl AgentFileSystem {
    /// Open a file, returning a file handle. Acquires lock based on mode.
    /// O(1). Lock: shared for Read, exclusive for Write/Append.
    pub fn open(
        &self,
        path: &str,
        mode: OpenMode,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<FileHandle, FsError>;

    /// Read from an open file handle at current position.
    /// O(content_read). Lock: must hold shared lock (acquired by open).
    pub fn read(
        &self,
        handle: &mut FileHandle,
        buf: &mut [u8],
    ) -> Result<usize, FsError>;

    /// Write to an open file handle at current position.
    /// O(content_written). Lock: must hold exclusive lock.
    pub fn write(
        &self,
        handle: &mut FileHandle,
        buf: &[u8],
    ) -> Result<usize, FsError>;

    /// Seek to a position in the file.
    /// O(1).
    pub fn lseek(
        &self,
        handle: &mut FileHandle,
        offset: isize,
        whence: Whence,
    ) -> Result<usize, FsError>;

    /// Close a file handle. Releases lock.
    /// O(1).
    pub fn close(
        &self,
        handle: FileHandle,
    ) -> Result<(), FsError>;

    /// List directory contents. O(k) where k = entries.
    pub fn list(
        &self,
        path: &str,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<Vec<String>, FsError>;

    /// Delete a file. O(1). Acquires exclusive lock.
    pub fn delete(
        &self,
        path: &str,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<(), FsError>;
}
```

### 6.2 Transactions (Atomic Multi-file Operations)

```rust
impl AgentFileSystem {
    /// Begin a new transaction.
    pub fn begin_transaction(
        &self,
        caller: AgentId,
        trust_level: SovereigntyLevel,
    ) -> Result<Transaction, FsError>;

    /// Stage a write operation within a transaction (not yet applied).
    pub fn tx_write(
        &self,
        tx: &mut Transaction,
        path: &str,
        content: &[u8],
    ) -> Result<(), FsError>;

    /// Stage a delete operation within a transaction.
    pub fn tx_delete(
        &self,
        tx: &mut Transaction,
        path: &str,
    ) -> Result<(), FsError>;

    /// Stage an export operation within a transaction.
    pub fn tx_export(
        &self,
        tx: &mut Transaction,
        from: &str,
        to: &str,
    ) -> Result<(), FsError>;

    /// Commit transaction atomically. All operations succeed or all fail.
    /// Acquires exclusive locks on all affected files, applies operations,
    /// releases locks. If conflict detected, aborts and returns error.
    pub fn commit_transaction(
        &self,
        tx: Transaction,
    ) -> Result<(), FsError>;

    /// Rollback transaction. Discards all staged operations.
    pub fn rollback_transaction(
        &self,
        tx: Transaction,
    );
}
```

### 6.3 Fine-grained Sensitive Operation Restrictions

Sensitive operations have **additional restrictions beyond TrustLevel**:

```rust
pub enum SensitiveOperation {
    DeleteSharedFile,        // Delete /shared file
    ModifySharedFile,        // Modify /shared file (not by owner)
    ExportToShared,          // Export from /home to /shared
    DeletePrivateFile,       // Delete own /home file
    CreateSharedSubdir,      // Create /shared subdirectory
}

// Fine-grained permission matrix:
// | Operation            | TrustLevel | Additional Restriction           |
// |----------------------|------------|----------------------------------|
// | DeleteSharedFile     | L3         | Must be owner OR admin           |
// | ModifySharedFile     | L2         | Must be owner OR listed collab   |
// | ExportToShared       | L2         | None                             |
// | DeletePrivateFile    | L0         | Must be owner                    |
// | CreateSharedSubdir   | L3         | None                             |
```

### 6.4 Audit Log (Optional Extension)

All sensitive operations and transaction commits are logged:

```rust
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub agent_id: AgentId,
    pub operation: String,
    pub path: String,
    pub success: bool,
    pub trust_level: u8,
    pub transaction_id: Option<u64>,
}

impl AgentFileSystem {
    /// Get audit log entries for a specific agent.
    pub fn audit_for_agent(&self, agent_id: AgentId) -> Vec<&AuditEntry>;

    /// Get audit log entries for a specific path.
    pub fn audit_for_path(&self, path: &str) -> Vec<&AuditEntry>;
}
```

---

## 7. Integration with Existing Systems

### 7.1 MemoryTool Thin Wrapper

The existing `MemoryTool` becomes a thin facade over AFS, using the POSIX-style API:
```rust
impl MemoryTool {
    // Old API preserved for backward compatibility:
    pub fn store(&self, key: &str, value: Value) {
        let path = format!("/home/{}/{}", self.agent_id, key);
        let content = serde_json::to_vec(&value).unwrap();
        
        let mut handle = self.fs.open(
            &path,
            OpenMode::Write,
            self.agent_id,
            SovereigntyLevel::Level0,
        ).unwrap();
        
        handle.write(&content).unwrap();
        self.fs.close(handle).unwrap();
    }

    pub fn retrieve(&self, key: &str) -> Option<Value> {
        let path = format!("/home/{}/{}", self.agent_id, key);
        
        let mut handle = self.fs.open(
            &path,
            OpenMode::Read,
            self.agent_id,
            SovereigntyLevel::Level0,
        ).ok()?;
        
        let mut buf = Vec::new();
        handle.read(&mut buf).ok()?;
        self.fs.close(handle).ok()?;
        
        serde_json::from_slice(&buf).ok()
    }
}
```

This means `experiments/personal_assistant.rs` continues to work unchanged.

### 7.2 Runtime Integration

The Runtime holds the AFS instance and provides access to agents:
```rust
pub struct Runtime {
    records: HashMap<AgentId, AgentRecord>,
    scheduler: Box<dyn Scheduler>,
    ipc_broker: IpcBroker,
    file_system: Arc<AgentFileSystem>,  // NEW
}
```

### 7.3 Sovereignty Integration

AFS permission checks use the `SovereigntyLevel` from `TrustMeter`:
```rust
// In AFS read/write:
let required_level = self.required_level_for(path, operation);
if (trust_level.as_u8()) < required_level {
    return Err(FsError::PermissionDenied {
        required: required_level,
        current: trust_level.as_u8(),
    });
}
```

---

## 8. Validation Plan

### 8.1 Unit Tests

- [ ] Path parsing (valid/invalid paths)
- [ ] Virtual file generation (/proc reads)
- [ ] Private file read/write via POSIX API (open/read/write/close/lseek)
- [ ] Shared file read/write via POSIX API
- [ ] Permission enforcement per TrustLevel (all 4 levels × 5 paths)
- [ ] **Fine-grained sensitive operation restrictions** (5 operations × permission matrix)
- [ ] **File locking**: shared lock for readers, exclusive for writers
- [ ] **Concurrent read/write safety**: multiple readers OK, writer blocks readers
- [ ] **Transaction atomicity**: multi-file write all-succeed or all-fail
- [ ] **Transaction conflict detection**: abort on lock conflict
- [ ] Export operation with collaborator lists
- [ ] List directory
- [ ] Audit log recording

### 8.2 Integration Tests

- [ ] MemoryTool thin wrapper preserves existing behavior
- [ ] personal_assistant experiment passes unchanged
- [ ] TrustLevel progression unlocks AFS capabilities
- [ ] Concurrent agents reading same file (no blocking)
- [ ] Concurrent agents writing same file (exclusive lock enforced)
- [ ] Transaction rollback on conflict

### 8.3 Experiment: `experiments/afs_experiment.rs`

Demonstrates:
1. **Virtual /proc reads**: Agent reads its own trust score, intent, metrics via POSIX API
2. **Private /home write**: Agent opens, writes, seeks, reads, closes
3. **Cross-agent observation**: Level 1 agent reads another agent's /proc
4. **Collaboration via /shared**: Level 2 agent exports private file, another agent reads it
5. **Permission denied**: Level 0 agent tries to access /shared → FsError::PermissionDenied
6. **Fine-grained restriction**: Level 2 agent tries to delete shared file → FsError::SensitiveOpDenied
7. **Transaction atomicity**: Write 3 files in a transaction, verify all-or-nothing
8. **Lock contention**: Two agents try to write same file, verify exclusive lock works
9. **Performance benchmark**: Measure open/read/write/close latencies, verify targets

Validation criteria:
1. ✓ POSIX API (open/read/write/close/lseek) works correctly
2. ✓ Virtual /proc reads are O(1), zero storage overhead
3. ✓ Private storage read/write works with correct lock behavior
4. ✓ TrustLevel gates access correctly (4 levels × 5 operations)
5. ✓ Fine-grained restrictions enforced (5 sensitive operations)
6. ✓ Transactions are atomic (all-or-nothing)
7. ✓ Lock contention handled correctly (exclusive lock for writes)
8. ✓ Export operation copies content, preserves metadata, sets collaborators
9. ✓ Concurrent reads do not block each other
10. ✓ MemoryTool backward compatibility (personal_assistant passes)

---

## 9. Open Questions

| Question | Why it matters | Current leaning |
|----------|----------------|-----------------|
| Persistence to disk? | Research project vs production use | **Start in-memory only**, add persistence hook via trait |
| Directory creation (mkdir)? | Needed for shared space organization | **Yes, but only for L3+** |
| File versioning? | Useful for undo/audit | **Defer to future RFC** |
| Watch mechanism (inotify)? | Notify agents of file changes | **Defer to future RFC** |
| Hardlinks/symlinks? | Advanced filesystem feature | **No — too complex for v1** |
| Per-file ACLs beyond TrustLevel? | Fine-grained control | **Yes — via collaborator lists** (see §3.3) |
| Deadlock detection for transactions? | Safety guarantee | **Use two-phase locking (2PL)** — deadlock-free by design |
| Large file support? | Performance for big content | **Start with Vec<u8>**, defer mmap-style streaming |

## 10. Design Review Summary

**Incorporated feedback from user**:

| Feedback Area | Adjustment Made |
|---------------|-----------------|
| Data consistency | Added **per-file RwLock** + **two-phase locking transactions** |
| Sensitive operations | Added **fine-grained restriction matrix** + **collaborator lists** |
| POSIX-style API | Replaced simple read/write with **open/read/write/close/lseek** |
| Compatibility | **MemoryTool thin wrapper** preserves old API |
| Performance | Lock-free /proc, concurrent reads, O(1) operations |
| Path structure | Kept /proc + /home + /shared (three layers) |

---

## 10. References

- `rfc/001-agent-process.md` — Agent abstraction, AgentRecord
- `rfc/002-scheduling.md` — Scheduling, Runtime structure
- `rfc/003-sovereignty-system.md` — TrustMeter, SovereigntyLevel, Kernel/User Space
- Linux procfs documentation — Virtual filesystem inspiration
- `runtime/src/tool/memory_tool.rs` — Existing MemoryTool to be replaced

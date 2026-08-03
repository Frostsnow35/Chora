# Negotiation IPC & Runtime 集成 Spec

## Why
NegotiationEngine 目前是独立的模块，与 Runtime 调度循环和 IPC 子系统未集成。要实现真正的多主权 Agent 协作，需要：
1. 通过 IPC 消息驱动协商流程（提案分发、响应收集、投票）
2. Runtime 能够管理协商会话的生命周期（超时检查、清理）

## What Changes
- 在 IPC 子系统中定义协商消息类型
- NegotiationEngine 提供 IPC-driven 的协商接口
- Runtime 集成 NegotiationEngine，在调度循环中管理协商
- 添加 StepOutput::Negotiation 类型让 Agent 触发协商
- 更新 Runtime 主循环定期检查协商状态

## Impact
- Affected specs: IPC 子系统、Runtime 调度、RFC-005 协商协议
- Affected code: `runtime/src/ipc/mod.rs`, `runtime/src/scheduling/runtime.rs`, `runtime/src/negotiation/mod.rs`, `runtime/src/lib.rs`

## ADDED Requirements

### Requirement: IPC 协商消息类型
系统应提供 IPC 消息类型用于协商协议各阶段的通信。

#### Scenario: 提案分发
- **WHEN** Level 3 Agent 提出修改 ExecutionPlan
- **THEN** 系统通过 IPC 广播 ProposalBroadcast 消息给所有 affected_agents

#### Scenario: 响应收集
- **WHEN** affected_agent 评估提案后提交响应
- **THEN** 系统通过 IPC 发送 ResponseMessage 给 proposer

#### Scenario: 投票阶段
- **WHEN** counter-proposal 解析完成进入最终投票
- **THEN** 系统通过 IPC 广播 VotingStarted 消息通知所有参与者

### Requirement: NegotiationEngine IPC 集成
NegotiationEngine 应能通过 IPC 接收和发送协商消息。

#### Scenario: 通过 IPC 创建协商会话
- **WHEN** Agent 通过 IPC 发送 CreateProposalRequest
- **THEN** NegotiationEngine 创建会话并通过 IPC 返回 SessionId

#### Scenario: 通过 IPC 提交响应和投票
- **WHEN** Agent 通过 IPC 发送 ResponseMessage 或 VoteMessage
- **THEN** NegotiationEngine 处理消息并更新会话状态

### Requirement: Runtime 协商管理
Runtime 应集成 NegotiationEngine 并在调度循环中管理协商会话。

#### Scenario: 协商生命周期管理
- **WHEN** Runtime 执行调度循环
- **THEN** 定期调用 check_all_timeouts() 和 cleanup_completed()

#### Scenario: Agent 触发协商
- **WHEN** Agent step() 返回 StepOutput::NegotiationRequest
- **THEN** Runtime 转发给 NegotiationEngine 创建会话

## MODIFIED Requirements

### Requirement: StepOutput 扩展
现有的 StepOutput enum 需要新增协商相关类型。

原有:
```rust
pub enum StepOutput {
    Text(String),
    ToolCall(ToolCallRequest),
    Communication(Message),
    StateChange(SchedulingState),
    MemoryOperation(MemoryOp),
    SendIpc { channel_id, message },
    CreateChannel { channel_type, receivers, capacity },
}
```

新增:
```rust
pub enum StepOutput {
    // ... existing types ...
    /// Request to create a negotiation session
    NegotiationRequest {
        target_agent: AgentId,
        affected_agents: Vec<AgentId>,
        amendment: PlanAmendment,
        rationale: String,
    },
    /// Submit response to an active negotiation
    NegotiationResponse {
        session_id: SessionId,
        response: InitialResponse,
    },
    /// Submit vote in final voting phase
    NegotiationVote {
        session_id: SessionId,
        decision: VoteDecision,
        rationale: String,
    },
}
```
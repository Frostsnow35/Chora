# Tasks

- [x] Task 1: IPC 协商消息类型定义
  - [x] SubTask 1.1: 在 `runtime/src/ipc/negotiation_messages.rs` 中定义消息类型
  - [x] SubTask 1.2: 定义 NegotiationMessage enum（ProposalBroadcast, ResponseMessage, VoteMessage, VotingStarted 等）
  - [x] SubTask 1.3: 为 NegotiationMessage 实现 Serialize/Deserialize
  - [x] SubTask 1.4: 在 `runtime/src/ipc/mod.rs` 中导出 NegotiationMessage

- [x] Task 2: NegotiationEngine IPC 集成
  - [x] SubTask 2.1: 在 NegotiationEngine 中添加 handle_ipc_message 方法
  - [x] SubTask 2.2: 实现 create_session_from_ipc 处理 CreateProposalRequest
  - [x] SubTask 2.3: 实现 handle_response_from_ipc 处理 ResponseMessage
  - [x] SubTask 2.4: 实现 handle_vote_from_ipc 处理 VoteMessage
  - [x] SubTask 2.5: 添加 broadcast_proposal 方法通过 IPC 广播提案
  - [x] SubTask 2.6: 添加 broadcast_voting_started 方法通知投票开始

- [x] Task 3: StepOutput 协商类型扩展
  - [x] SubTask 3.1: 在 `runtime/src/lib.rs` 的 StepOutput enum 中添加 NegotiationRequest
  - [x] SubTask 3.2: 添加 NegotiationResponse 类型
  - [x] SubTask 3.3: 添加 NegotiationVote 类型
  - [x] SubTask 3.4: 导出 negotiation 类型供 Agent 使用

- [x] Task 4: Runtime NegotiationEngine 集成
  - [x] SubTask 4.1: 在 Runtime struct 中添加 negotiation_engine 字段
  - [x] SubTask 4.2: 实现 Runtime::with_negotiation 构造函数
  - [x] SubTask 4.3: 在 run_step 方法中处理 StepOutput::NegotiationRequest
  - [x] SubTask 4.4: 在 run_step 方法中处理 StepOutput::NegotiationResponse
  - [x] SubTask 4.5: 在 run_step 方法中处理 StepOutput::NegotiationVote
  - [x] SubTask 4.6: 在主循环中添加 negotiate_tick() 定期检查超时和清理

- [x] Task 5: 测试验证
  - [x] SubTask 5.1: 测试 IPC 协商消息序列化和反序列化
  - [x] SubTask 5.2: 测试 NegotiationEngine.handle_ipc_message 处理流程
  - [x] SubTask 5.3: 测试 Runtime 处理 NegotiationRequest
  - [x] SubTask 5.4: 测试完整的 IPC-driven 协商流程
  - [x] SubTask 5.5: 测试 Runtime negotiate_tick() 超时检查

# Task Dependencies
- [Task 2] depends on [Task 1]
- [Task 3] 无依赖，可与 Task 1 并行
- [Task 4] depends on [Task 2] 和 [Task 3]
- [Task 5] depends on [Task 4]
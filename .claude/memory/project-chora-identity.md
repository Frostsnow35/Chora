---
name: project-chora-identity
description: Chora 项目的核心定位、命名来源、当前状态
metadata:
  type: project
---

# Chora - OS 启发的 AI 代理运行时框架

## 项目名称含义
- **Chora** 源自希腊哲学中的 **χώρα**（khôra），意为"原始场域/容器"
- 哲学出处：柏拉图《蒂迈欧篇》——先于形式、容纳一切形式的接收者（receptacle）
- 哲学寓意：**场域先于任何具体形式而存在**，使形式得以生发，但不规定形式本身
- 对应技术哲学：Runtime 作为"场域"容纳多主权 Agent 共生，提供协作发生的基础设施（IPC、AFS、协商协议），但不规定 Agent 如何协作——形式从多主体的协商中自然涌现

## 核心定位
**三轨并行**：
1. **开源库**：让其他开发者基于 Chora 构建多 Agent 应用（API 清晰、集成容易）
2. **研究原型**：产出论文/技术报告，指导真实系统设计（理论深度、实验验证）
3. **产品化**：最终形成真实用户场景的产品（可用、可靠、有独特价值）

当前阶段（2026-06-28）：研究原型 + 开源库基础建设，产品化在后期

## 当前状态（2026-06-27）
**GitHub**: https://github.com/Frostsnow35/Chora
**Owner**: 梁隽维 (Frostsnow35)
**Language**: Rust (async, Tokio)
**License**: MIT

### RFC 进度
| RFC | 主题 | 状态 |
|-----|------|------|
| RFC-001 | Agent 抽象 + 三验证代理 | ✅ 完成（3/3 验证代理通过） |
| RFC-002 | 调度系统 | ✅ 完成（Scheduler trait + FifoScheduler + Runtime + 验证代理通过） |
| RFC-003 | 主权系统（成长型自治） | ✅ 完成（已合并到 master） |
| RFC-004 | Agent File System（AFS） | ✅ 完成（POSIX API + /proc 虚拟文件 + 权限控制，基础修复待做） |
| Q3实验 | Trust Score → LLM 参数映射 | ✅ 完成（3策略 + 验证代理通过） |
| IPC | 进程间通信（P2P、广播、背压） | ✅ 完成（Channel + Broker 已实现） |
| **RFC-005** | **多主权协商协议（Intent Negotiation Protocol）** | 🎯 **下一方向（待启动）** |

### 已实现组件
- `runtime/src/` - Agent trait, Intent, AgentProgram, AgentRecord, ToolExecutor, StepContext
- `runtime/src/scheduling/` - Scheduler trait, FifoScheduler, Runtime (RFC-002 协作式调度)
- `runtime/src/ipc/` - IPC 子系统：ChannelId, Channel trait, UnidirectionalChannel (P2P), BroadcastChannel (一对多), IpcBroker (消息路由)
- `runtime/src/fs/` - Agent File System (RFC-004): POSIX API (open/read/write/close/lseek), /proc 虚拟文件, /home 私有存储, /shared 共享存储, TrustLevel 权限控制, RwLock 并发
- `runtime/src/sovereignty/` - TrustMeter, SovereigntyGate, IntentCore, SovereignAgentImpl
- `runtime/src/sovereignty/reasoning_params.rs` - ReasoningConfig + 3种映射策略 + ReasoningMapper
- `runtime/src/sovereignty/reasoning_engine.rs` - SimulatedLLM 模拟推理引擎
- `runtime/src/tool/memory_tool.rs` - MemoryTool + MemoryStore
- `experiments/calculator.rs` - Agent 1: Tool-Using 验证
- `experiments/pure_reasoning.rs` - Agent 2: Pure Reasoning 验证
- `experiments/personal_assistant.rs` - Agent 3: Memory-Dependent 验证
- `experiments/sovereignty_llm_experiment.rs` - Q3 验证: Trust Score → LLM 参数映射
- `experiments/scheduling_experiment.rs` - RFC-002 验证: 协作式调度（FIFO, yield, block, terminate）
- `experiments/ipc_experiment.rs` - IPC 验证: P2P, 广播, 背压, Runtime 集成
- `experiments/afs_experiment.rs` - RFC-004 验证: POSIX API, /proc 虚拟文件, /home 私有, 权限控制

## 下一方向：多主权协商（RFC-005）
**突破口**：多主权 Agent 的**真实协作**（不是 IPC 消息，而是真正的协商）
- **Intent Negotiation Protocol（意图协商协议）**：Level 3 Agent 提出 AmendmentProposal，受影响 Agent 基于宪法评估，加权投票/共识/否决
- **双边信任模型**：A→B 的定向信任（非全局），基于协作历史演化，影响 IPC 带宽、共享空间访问、协商权重
- **端到端验证场景**：软件开发团队（架构/实现/测试 Agent 协商需求变更），演示 Level 3 提议 → 协商 → 共识 → 执行 完整流程

**哲学映射**：
- Runtime = 场域（让多主体共存的"空"空间）
- 协商协议 = 场域的"律"（允许形式生成但不规定形式）
- 多 Agent 协作 = 场域中形式的具体化

## 关键约束
- Agent trait signature 已冻结（RFC-001）
- 协作式调度，永不抢占（preemptive scheduling 不适用）
- 内核态保护（Kernel Space 类型不对外暴露）
- 信任分数更新必须对称设计（慢涨快跌）

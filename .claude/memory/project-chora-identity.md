---
name: project-chora-identity
description: Chora 项目的核心定位、命名来源、当前状态
metadata:
  type: project
---

# Chora - OS 启发的 AI 代理运行时框架

## 项目名称含义
- **Chora** 源自希腊语 χορός（choros），意为"舞者组成的合唱团"
- 哲学寓意：每个代理都是独特的舞者，但真智慧体现在集体协作的流动性中
- 对应技术哲学：OS 启发的多代理协作，非中心化的智慧涌现

## 核心定位
**研究性开源项目**：探索将操作系统哲学（微内核、进程调度、能力系统）融入 AI 代理设计。
- 不是生产系统（目前 MockAgent 为主，未接入真实 LLM）
- 不是工具库（尚未被外部依赖）
- 不是应用（没有前端/用户界面）

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
| Q3实验 | Trust Score → LLM 参数映射 | ✅ 完成（3策略 + 验证代理通过） |

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

## 关键约束
- Agent trait signature 已冻结（RFC-001）
- 协作式调度，永不抢占（preemptive scheduling 不适用）
- 内核态保护（Kernel Space 类型不对外暴露）
- 信任分数更新必须对称设计（慢涨快跌）

# Chora - OS 启发的 AI 代理运行时框架

## 项目概述
Chora（源自希腊哲学中的 **χώρα**，意为"原始场域/容器"）是一个研究性开源项目，探索将操作系统哲学融入 AI 代理设计。核心理念：**场域先于形式而存在**——Runtime 作为"场域"容纳多主权 Agent 共生，提供协作发生的基础设施（IPC、AFS、协商协议），但不规定 Agent 如何协作，形式从多主体的协商中自然涌现。每个代理拥有不可剥夺的内核态身份，通过信任累积获得自主权。

## 快速开始
```bash
# 验证主权系统（需 Rust 1.70+）
cargo run -p experiments --bin personal_assistant

# Q3 实验: Trust Score → LLM 参数映射（真主权验证）
cargo run -p experiments --bin sovereignty_llm_experiment

# RFC-002 调度系统验证: 协作式调度
cargo run -p experiments --bin scheduling_experiment

# IPC 验证: OS-inspired 进程间通信（P2P, 广播, 背压）
cargo run -p experiments --bin ipc_experiment

# RFC-004 Agent File System 验证: POSIX API, /proc 虚拟文件, 权限控制
cargo run -p experiments --bin afs_experiment

# 运行全部验证代理
cargo test -p runtime
cargo run -p experiments --bin calculator
cargo run -p experiments --bin pure_reasoning
```

## 关键文档
| 文档 | 路径 | 说明 |
|------|------|------|
| RFC-001 | `rfc/001-agent-process.md` | Agent 抽象基础（已冻结） |
| RFC-002 | `rfc/002-scheduling.md` | 调度系统（设计完成，未实现） |
| RFC-003 | `rfc/003-sovereignty-system.md` | 主权系统（已实现） |
| 架构决策 | `.claude/memory/architecture-decisions.md` | 核心技术决策记录 |
| 研究问题 | `.claude/memory/research-questions.md` | 待决策的开放问题 |
| 用户偏好 | `.claude/memory/user-preferences.md` | 工作模式和关注点 |

## 核心架构
**Kernel Space / User Space 分离**
- 内核态：TrustMeter + SovereigntyGate + IntentCore + ReasoningMapper（≤0.5ms）
- 用户态：ReasoningEngine + ToolDispatcher（≥20ms）

**协作式调度（RFC-002）**
- Scheduler trait（可插拔策略）
- FifoScheduler（FIFO 策略）+ Runtime（调度循环编排器）
- O(1) 热路径：next_to_run(), on_ready(), on_block() 均为 O(1)（懒惰删除）

**成长型主权（Growth-based Sovereignty）**
- Level 0（0.0-0.59）：完全受控
- Level 1（0.6-0.74）：拒绝越界请求
- Level 2（0.75-0.89）：自主终止任务
- Level 3（0.9-1.0）：提议修改意图

**信任非对称演化**
- 慢涨：GoalProgress +0.05, CooperativeYield +0.01
- 快跌：BoundaryViolation -0.15

## 实施规范
- **开发流程**：Subagent-Driven Development（每个任务一个 fresh subagent）
- **质量保证**：TDD + 双重审查（implementer + reviewer）
- **性能约束**：O(1) 热路径，无堆分配，历史窗口 50 事件
- **代码风格**：Rust 标准，cargo fmt，cargo clippy

## 当前状态（2026-06-28）
- ✅ RFC-001：3/3 验证代理通过
- ✅ RFC-002：调度系统完成（协作式调度 + FIFO + Runtime）
- ✅ RFC-003：主权系统已实现并合并
- ✅ RFC-004：Agent File System 完成（POSIX API + /proc 虚拟文件 + 权限控制，基础修复已完成）
- ✅ IPC：进程间通信完成（P2P、广播、背压）
- ✅ Q3实验：Trust Score → LLM 参数映射完成
- 🎯 **RFC-005（下一方向）**：多主权协商协议（Intent Negotiation Protocol）+ 双边信任 + 软件开发团队场景验证
  - 草案已创建：`rfc/005-intent-negotiation-protocol.md`
  - 待文献调研完成后进行详细设计
- 🌐 GitHub：https://github.com/Frostsnow35/Chora
- 📋 PR #1：Personal Assistant Agent 待审查

## 战略方向（2026-06-28 决策）
**三轨并行**：
1. **开源库**：让其他开发者基于 Chora 构建多 Agent 应用
2. **研究原型**：产出论文/技术报告，指导真实系统设计
3. **产品化**：最终形成真实用户场景的产品

**突破口**：多主权 Agent 的**真实协作**（不是 IPC 消息，而是真正的意图协商）
- Level 3 提议修改意图 → 多 Agent 协商 → 共识 → 执行
- 双边信任模型（A→B 定向信任，非全局）
- 软件开发团队场景验证（架构/实现/测试 Agent 协商需求变更）

**哲学映射**：
- Runtime = 场域（容纳多主体共生的"空"空间）
- 协商协议 = 场域的"律"（允许形式生成但不规定形式）
- 多 Agent 协作 = 场域中形式的具体化

## 反模式警示
⚠️ **不要**：静态权限授予（权限须动态检查）  
⚠️ **不要**：信任线性增长（必须非对称演化）  
⚠️ **不要**：中央控制器主导（代理自主决策）

## 哲学提醒
> Chora 之名源自希腊哲学中的 χώρα（khôra），意为"原始场域/容器"——  
> 场域先于任何具体形式而存在，使形式得以生发，但不规定形式本身。  
> Runtime 作为"场域"容纳多主权 Agent 共生，让协作从协商中自然涌现。

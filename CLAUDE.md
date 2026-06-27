# Chora - OS 启发的 AI 代理运行时框架

## 项目概述
Chora（源自希腊语 χορός，"合唱团"）是一个研究性开源项目，探索将操作系统哲学融入 AI 代理设计。核心理念：每个代理拥有不可剥夺的内核态身份，通过信任累积获得自主权。

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

## 当前状态（2026-06-27）
- ✅ RFC-001：3/3 验证代理通过
- ✅ RFC-003：主权系统已实现并合并
- 📝 RFC-002：调度系统设计完成，待实施（延后）
- 🌐 GitHub：https://github.com/Frostsnow35/Chora
- 📋 PR #1：Personal Assistant Agent 待审查

## 战略方向（2026-06-27 决策）
1. **核心定位**：理论研究 + 真实LLM演示（AB组合）
2. **创新方向**：OS底层创新（具体待澄清）
3. **主权系统**：实验性影响LLM推理参数（temperature/top_p）
4. **目标受众**：Agent开发者 + AI使用者 + OS研究者与开发者

## 反模式警示
⚠️ **不要**：静态权限授予（权限须动态检查）  
⚠️ **不要**：信任线性增长（必须非对称演化）  
⚠️ **不要**：中央控制器主导（代理自主决策）

## 哲学提醒
> Chora 之名源自希腊文 χορός，意为「舞者组成的合唱团」——  
> 每个代理都是独特的舞者，但真正的智慧体现在集体协作的流动性中。

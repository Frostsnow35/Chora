---
name: architecture-decisions
description: Chora 项目的核心技术决策和架构选择
metadata:
  type: project
---

# 架构决策记录

## 1. 主权系统设计（RFC-003）

### 核心架构：Kernel Space / User Space 分离
**决策**：每个 Agent 内部有微内核结构
- **内核态**（Kernel Space）：IntentCore + TrustMeter + SovereigntyGate
- **用户态**（User Space）：ReasoningEngine + ToolDispatcher
- **保护机制**：User Space 只能读 Trust Score，不能修改

### 主权模型：成长型主权（Growth-based Sovereignty）
**决策**：组合 B + C
- **B - 自我终止与演化**：Agent 可判断目标达成/不可能并自主终止
- **C - 协作中的主权**：Agent 间是对等合作关系，非主从命令

### 主权来源：动态授予
**决策**：组合 B + C
- **B - 逐步授予**：初始权限小，随任务推进解锁
- **C - 协商获得**：通过表现动态赢得信任，可失去

### Trust Meter 参数
```rust
// 非对称更新（慢涨快跌）
GoalProgress => +0.05,           // 稳健自主需 10x 时间
SovereignActionApproved => +0.02,
CooperativeYield => +0.01,
SovereignActionDenied => -0.01,
BoundaryViolation => -0.15,      // 信任崩坏只需 1x

// 主权级别阈值
Level 0: 0.0-0.59  // 完全受控
Level 1: 0.6-0.74  // 拒绝越界请求
Level 2: 0.75-0.89 // 自主终止任务
Level 3: 0.9-1.0   // 提议修改意图
```

### 性能约束
- Trust Score 读取：O(1)
- 主权级别检查：O(1)
- 历史窗口：50 个事件（防止无限增长）
- 热路径无堆分配（hot path = score read, level check）

## 2. Intent 双层结构
**决策**：Constitution（不可变，append-only）+ ExecutionPlan（可变）
- **Constitution**：Agent 的根本身份，spawn 时确定
- **ExecutionPlan**：当前目标和策略，User Space 可更新
- **修正机制**：只能追加边界，不能删除已有边界

## 3. 调度模型（RFC-002）
**决策**：协作式调度（Cooperative Scheduling）
- 永不抢占：Agent 必须主动 yield
- 调度单位 = LLM 推理预算（token + wall-clock）
- 优先级：动态（基于 Intent 约束）
- 调度策略可插拔（P1: 机制/策略分离）

## 4. 技术栈选择
- **语言**：Rust（安全、性能、并发）
- **异步**：Tokio
- **序列化**：serde
- **错误处理**：thiserror
- **测试**：cargo test + MockAgent

## 5. 实现策略
- **Subagent-Driven Development**：每个任务一个 fresh subagent
- **TDD**：先写失败测试，再实现
- **RFC 流程**：设计 → 实施 → 验证代理 → 合并
- **质量保证**：implementer + reviewer subagents 双重把关

## 6. 多主权协商协议（RFC-005，2026-06-28 决策）

### 战略定位：三轨并行
**决策**：开源库 + 研究原型 + 产品化同时推进
- **开源库**：让其他开发者基于 Chora 构建多 Agent 应用（API 清晰、集成容易）
- **研究原型**：产出论文/技术报告，指导真实系统设计（理论深度、实验验证）
- **产品化**：最终形成真实用户场景的产品（可用、可靠、有独特价值）

### 突破口：多主权 Agent 的真实协作
**决策**：聚焦"多主权协商"，不是 IPC 消息，而是真正的意图协商
- **Intent Negotiation Protocol（意图协商协议）**：
  - Level 3 Agent 发现原 Intent 有问题 → 发出 `AmendmentProposal`
  - 受影响的相关 Agent 收到提案 → 基于自身宪法评估
  - 协商机制：加权投票 / 共识 / 否决权
  - 失败处理：维持原意图 / 分裂协作组
- **双边信任模型**：
  - `A→B` 的定向信任（非全局 Trust Score）
  - 基于协作历史演化（B 多次遵守承诺 → A 对 B 信任上升）
  - 影响 IPC 带宽、共享空间访问、协商权重
- **端到端验证场景**：软件开发团队（架构/实现/测试 Agent 协商需求变更）

### 哲学映射（场域概念）
**决策**：Runtime = 场域，协商协议 = 场域的"律"
- **Runtime** = 场域（让多主体共存的"空"空间，先于形式、容纳形式）
- **协商协议** = 场域的"律"（允许形式生成但不规定形式）
- **多 Agent 协作** = 场域中形式的具体化（从多主体协商中自然涌现）
- **避免**：不复制 OS 结构（不引入设备驱动、内存分区等硬件抽象）

### 竞品差异化
- **LangChain / AutoGen / CrewAI**：Agent 是工具，协作是调度，信任是二元的
- **Chora**：Agent 是主权主体，协作是协商，信任是演化的
- **独特功能**：Level 3 提议修改意图 + 多 Agent 协商 + 双边信任

## 7. 设计哲学：场域而非容器
**决策**：Chora 的"场域"是**使形式成为可能的空间**，不是给 Agent 套的"容器"
- ✅ 场域：Runtime 提供基础设施（IPC、AFS、协商协议），让协作自然发生
- ❌ 容器：给 Agent 加外壳、限制其行为、强制某种结构
- ✅ 抽象映射：从 OS 哲学中提取原则（隔离、控制、共享），不复制具体组件
- ❌ 硬件抽象：不引入设备驱动、内存分区等 OS 组件

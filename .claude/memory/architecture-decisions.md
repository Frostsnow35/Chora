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

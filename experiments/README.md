# Chora Experiments

验证性代理和行为测试，用于验证 RFC 规范的实现。

## 运行实验

```bash
# 基础代理抽象
cargo run -p experiments --bin pure_reasoning

# 计算器代理
cargo run -p experiments --bin calculator

# 个人助手代理（主权系统演示）
cargo run -p experiments --bin personal_assistant

# 主权 LLM 实验（信任→LLM 参数映射）
cargo run -p experiments --bin sovereignty_llm_experiment

# 调度实验（协作式调度）
cargo run -p experiments --bin scheduling_experiment

# IPC 实验（P2P、广播、背压）
cargo run -p experiments --bin ipc_experiment

# AFS 实验（POSIX API、/proc、权限控制）
cargo run -p experiments --bin afs_experiment

# 协商协议实验（软件开发团队场景）
cargo run -p experiments --bin negotiation_experiment

# 人机协作实验（人类与 AI Agent 协商）
cargo run -p experiments --bin human_agent
```

## 实验说明

| 实验 | 验证内容 | RFC |
|------|----------|-----|
| `pure_reasoning` | 代理抽象基础 | RFC-001 |
| `calculator` | 工具调用能力 | RFC-001 |
| `personal_assistant` | 主权系统演化 | RFC-003 |
| `sovereignty_llm_experiment` | 信任→LLM 参数映射 | RFC-003 |
| `scheduling_experiment` | 协作式调度 | RFC-002 |
| `ipc_experiment` | 进程间通信 | RFC-002 |
| `afs_experiment` | Agent 文件系统 | RFC-004 |
| `negotiation_experiment` | 意图协商协议 | RFC-005 |
| `human_agent` | 人机协作协商 | RFC-005 |

## 测试

```bash
# 运行所有测试
cargo test -p experiments

# 运行特定模块测试
cargo test -p runtime --lib negotiation
cargo test -p runtime --lib sovereignty
cargo test -p runtime --lib scheduling
```
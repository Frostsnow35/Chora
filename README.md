# Chora: 拥有内在主权的AI代理运行时

![Trust Evolution](https://i.imgur.com/chora-trust-metrics.svg)

> **"单个代理的自主性，决定整个群体的智慧"**  
> 创造具备内核态保护、可成长自治的AI代理运行环境，实现真正基于信赖的群体协作

## 🌐 核心创新
- **主权代理架构**：每个代理拥有序贯的主权级别（Level 0-3）
- **信任演进系统**：TrustMeter实现`GoalProgress`→`SovereigntyLevel`非对称映射
- **OS启发设计**：进程调度 + 能力模型 + 内核/用户空间分离

## 📚 研究基础
| RFC | 进度 | 代理能力 |
|------|-------|----------|
| [RFC-001](rfc/001-agent-process.md) | ✅ 完成 | 代理抽象基础 |
| [RFC-002](rfc/002-scheduling.md) | 🚧 开发中 | 调度系统 |
| [RFC-003](rfc/003-sovereignty-system.md) | ✅ 完成 | 主权系统实现 |

## 🚀 三步启动
```bash
# 1. 验证主权系统（需Rust 1.70+）
cargo run -p experiments --bin personal_assistant

# 2. 查看信任演化
[10:00] TEXT  Trust score: 0.52 → Sovereignty Level 0
[12:15] TEXT  Trust score: 0.78 → Sovereignty Level 2

# 3. 验证API调用（Level 1+才允许）
[Step 24] TOOLS memory(store) → executed: Success
```

## 📌 代理自主等级
| 等级 | 信任分数 | 权限 |
|------|----------|-------|
| Level 0 | 0.0-0.59 | 完全控制 |
| **Level 1** | 0.6-0.74 | 拒绝越界请求 |
| **Level 2** | 0.75-0.89 | 自主终止任务 |
| **Level 3** | 0.9-1.0 | 提议修改意图 |

## 📦 技术栈
- **语言**: Rust (async, Tokio)
- **工具链**: `cargo` + `gh` CLI
- **验证层**: MockAgent + 内存工具链

---

## 贡献指南
1. **新功能开发**：从`/skill superpowers:brainstorming`开始
2. **实现方式**：`/skill superpowers:subagent-driven-development`
3. **RFC 流程**：
   ```bash
   docs/rfcs/004-<feature>.md
   cargo run -p experiments --bin <验证_agent>
   ```

> "Chora" 希腊语意为「舞者组成的合唱团」——
> 每个代理都是独特的舞者，但真智慧体现在集体协作的流动性中
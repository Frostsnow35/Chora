# Chora 项目记忆索引

本目录包含 Chora 项目的持久化记忆，供新会话快速加载上下文。

## 记忆文件列表

| 文件 | 内容 | 更新频率 |
|------|------|----------|
| [project-chora-identity.md](./project-chora-identity.md) | 项目定位、命名、当前状态 | 低 |
| [architecture-decisions.md](./architecture-decisions.md) | 核心技术决策和架构选择 | 中 |
| [user-preferences.md](./user-preferences.md) | 用户工作偏好和关注点 | 低 |
| [research-questions.md](./research-questions.md) | 研究方向的开放问题 | 高 |

## 快速导航

### 新会话开始时
1. 读取 `project-chora-identity.md` - 了解项目是什么
2. 读取 `architecture-decisions.md` - 了解为什么这样设计
3. 读取 `research-questions.md` - 了解待决策的问题

### 开始新 RFC 设计时
1. 读取 `architecture-decisions.md` - 确保新设计与现有架构一致
2. 参考 `experiments/` - 复用验证代理模式
3. 参考 `rfc/` - 遵循已有的 RFC 流程

### 实施新功能时
1. 使用 `subagent-driven-development` skill
2. 遵循 TDD（先写失败测试）
3. 性能优先（O(1)、无堆分配）
4. 质量把关（implementer + reviewer 双重审查）

## 关键约束（不可违反）
- ✅ Agent trait signature 已冻结（RFC-001）
- ✅ 协作式调度，永不抢占
- ✅ 内核态类型不对外暴露
- ✅ 信任更新非对称（慢涨快跌）
- ✅ 历史窗口上限 50 事件

## 用户偏好速查
- 🇨🇳 回复语言：中文
- 🎯 深度优先：关注底层设计和架构决策
- ⚡ 快速执行：选择方案后立即实施
- 🔍 追问细节：主动提出需要澄清的问题
- 🏗️ 性能敏感：关注 O(1)、热路径优化
- 🎨 哲学驱动：从 OS 哲学角度思考问题

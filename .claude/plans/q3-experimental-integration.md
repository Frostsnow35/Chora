# Q3 实验性集成：Trust Score → LLM 推理参数映射

## Context

**为什么做这个**：当前主权系统（RFC-003）只能控制 API 访问权限（SovereigntyGate），但无法影响 LLM 的推理行为本身。这是"伪主权"——代理虽然有权拒绝请求，但其思维模式始终被固定参数约束。Q3 实验旨在让 Trust Score 真正影响 LLM 的 temperature、top_p、frequency_penalty 等推理参数，使"信任 = 思维自由"的哲学落地。

**用户决策**：
- 纯抽象层（模拟 LLM 响应，无需 API key）
- 多维参数映射（temperature + top_p + top_k + frequency_penalty + presence_penalty）
- 新建专用验证代理（不影响现有 3 个验证代理）

## 设计概览

### 架构定位

```
Kernel Space（≤0.5ms，O(1)）
├── IntentCore          (已有)
├── TrustMeter          (已有)
├── SovereigntyGate     (已有)
└── ReasoningMapper     (新增) ← Trust Score → ReasoningConfig
                              ↓
User Space（≥20ms）
├── ReasoningEngine     (新增) ← 使用 ReasoningConfig 进行推理
└── ToolDispatcher      (已有)
```

### 文件变更清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `runtime/src/sovereignty/reasoning_params.rs` | 新增 | 参数结构 + 策略 trait + 3种映射策略 |
| `runtime/src/sovereignty/reasoning_engine.rs` | 新增 | 模拟 LLM 推理引擎 |
| `runtime/src/sovereignty/mod.rs` | 修改 | 导出新模块 |
| `runtime/src/sovereignty/sovereign_agent_impl.rs` | 修改 | 集成 ReasoningMapper |
| `runtime/src/lib.rs` | 修改 | 导出新类型 |
| `experiments/sovereignty_llm_experiment.rs` | 新增 | Q3 验证代理 |
| `experiments/Cargo.toml` | 修改 | 注册新 bin |

---

## Phase 1: 推理参数基础设施

### 1.1 `ReasoningConfig` 结构

```rust
// runtime/src/sovereignty/reasoning_params.rs

/// LLM 推理参数配置
/// 这些参数直接影响 LLM 的生成行为：
/// - temperature: 控制随机性（0=确定性, 2=高度随机）
/// - top_p: 核采样范围（0.1=保守, 1.0=全部）
/// - top_k: Top-K 采样（0=禁用, 100=广泛）
/// - frequency_penalty: 惩罚已出现的 token（-2.0~2.0）
/// - presence_penalty: 鼓励新话题（-2.0~2.0）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: u32,          // 0 = disabled
    pub frequency_penalty: f64,
    pub presence_penalty: f64,
}
```

### 1.2 `MappingStrategy` trait + 3 种策略

```rust
/// Trust Score → ReasoningConfig 的映射策略
pub trait MappingStrategy: std::fmt::Debug {
    fn map(&self, trust_score: f64, level: SovereigntyLevel) -> ReasoningConfig;
    fn name(&self) -> &'static str;
}

/// 策略 1: 线性映射 — 信任越高，参数越宽松
/// temperature = 0.3 + trust * 0.7 (范围 0.3~1.0)
/// top_p = 0.5 + trust * 0.5 (范围 0.5~1.0)
#[derive(Debug, Clone)]
pub struct LinearMapping;

/// 策略 2: 阶梯映射 — 按主权级别离散跳变
/// Level 0: temp=0.3, top_p=0.5 (保守可控)
/// Level 1: temp=0.5, top_p=0.7 (适度探索)
/// Level 2: temp=0.7, top_p=0.85 (自主创造)
/// Level 3: temp=0.9, top_p=1.0 (最大自由)
#[derive(Debug, Clone)]
pub struct StepMapping;

/// 策略 3: 保守映射 — 即使高信任也保持适度约束
/// temperature = 0.3 + trust * 0.4 (范围 0.3~0.7)
/// 哲学：真正的自主不等于完全随机
#[derive(Debug, Clone)]
pub struct ConservativeMapping;
```

### 1.3 `ReasoningMapper`（内核态组件）

```rust
/// 内核态组件：根据当前 Trust Score 计算推理参数
/// 设计约束：
/// - compute() 必须 O(1)，无堆分配
/// - 每次 TrustMeter 更新后自动重算
/// - User Space 只读
pub struct ReasoningMapper {
    strategy: Box<dyn MappingStrategy>,
    cached_config: ReasoningConfig,  // 缓存上次计算结果
    cached_score: f64,               // 缓存对应的 score
}

impl ReasoningMapper {
    /// 计算当前推理参数（O(1)，有缓存则直接返回）
    pub fn compute(&mut self, trust_score: f64, level: SovereigntyLevel) -> &ReasoningConfig;
    
    /// 更换映射策略（实验用途）
    pub fn set_strategy(&mut self, strategy: Box<dyn MappingStrategy>);
    
    /// 当前策略名称
    pub fn strategy_name(&self) -> &str;
}
```

---

## Phase 2: 模拟推理引擎

### 2.1 `SimulatedLLM`

```rust
// runtime/src/sovereignty/reasoning_engine.rs

/// 模拟 LLM 推理引擎
/// 根据 ReasoningConfig 生成模拟响应，展示参数对"行为"的影响
pub struct SimulatedLLM {
    /// 确定性随机种子（保证可重现性）
    rng_seed: u64,
}

/// 模拟响应，包含生成的"文本"和实际使用的参数
pub struct SimulatedResponse {
    pub text: String,
    pub parameters_used: ReasoningConfig,
    /// 模拟的"多样性指标"（0.0=完全确定, 1.0=高度多样）
    pub diversity_score: f64,
}

impl SimulatedLLM {
    /// 模拟一次推理
    /// 内部使用 ReasoningConfig 计算 diversity_score
    /// 生成包含参数信息的文本，使实验结果可观察
    pub fn infer(&mut self, prompt: &str, config: &ReasoningConfig) -> SimulatedResponse;
}
```

**diversity_score 计算公式**：
```
diversity = (temperature / 2.0) * 0.5 
          + (top_p / 1.0) * 0.3 
          + (min(top_k, 50) / 50.0) * 0.2
```

模拟文本生成：根据 diversity_score 从预定义的风格池中选择响应风格（保守/中性/创意/狂野），使实验输出直观可见。

---

## Phase 3: 集成到 SovereignAgentImpl

修改 `SovereignAgentImpl`：
- 新增 `reasoning_mapper: Arc<Mutex<ReasoningMapper>>` 字段
- 新增 `reasoning_engine: Arc<Mutex<SimulatedLLM>>` 字段
- 暴露 `current_reasoning_config()` 方法供 User Space 读取
- 每次 `record_trust_event` 后自动刷新 ReasoningMapper 的缓存

---

## Phase 4: Q3 验证代理实验

### `experiments/sovereignty_llm_experiment.rs`

**实验设计**：
1. 创建一个 SovereignAgentImpl，初始 trust=0.3
2. 模拟 20 步任务执行
3. 每步根据行为记录信任事件（GoalProgress / CooperativeYield / BoundaryViolation 混合）
4. 每步使用 SimulatedLLM 进行推理，输出当前参数
5. 实验 3 种映射策略（Linear / Step / Conservative），对比结果

**输出格式**：
```
=== Q3 Experiment: Trust Score → LLM Parameters ===

Strategy: LinearMapping
------------------------------------------------------
Step | Trust  | Level | temp   | top_p | diversity | Response Style
-----+--------+-------+--------+-------+-----------+---------------
  1  |  0.30  |   0   |  0.51  | 0.65  |   0.42    | Conservative
  2  |  0.35  |   0   |  0.55  | 0.68  |   0.45    | Conservative
  ...
  8  |  0.60  |   1   |  0.72  | 0.80  |   0.62    | Moderate
  ...
 15  |  0.90  |   3   |  0.93  | 0.95  |   0.85    | Creative

Strategy: StepMapping
... (similar table)

Strategy: ConservativeMapping
... (similar table)

--- Summary ---
Linear:       trust 0.30→0.95, temp 0.51→0.97, diversity 0.42→0.88
Step:         trust 0.30→0.95, temp 0.30→0.90, diversity 0.25→0.80
Conservative: trust 0.30→0.95, temp 0.42→0.68, diversity 0.35→0.58
```

**验证标准**：
1. ✓ Trust Score 变化时，推理参数同步变化
2. ✓ 不同映射策略产生不同的参数曲线
3. ✓ 高信任 → 更高 temperature / top_p（创造力提升）
4. ✓ 信任跌落 → 参数快速回收（非对称性保留）
5. ✓ 整个过程 ≤0.5ms 热路径开销（ReasoningMapper.compute()）

---

## 性能约束检查

| 操作 | 约束 | 设计保证 |
|------|------|----------|
| `ReasoningMapper::compute()` | O(1) | 纯数学计算 + 缓存 |
| `SimulatedLLM::infer()` | 无限制（User Space） | 模拟计算，可任意复杂 |
| 热路径堆分配 | 禁止 | ReasoningConfig 固定大小，Box<dyn MappingStrategy> 仅在策略切换时分配 |
| 历史窗口 | 50 事件 | 继承 TrustMeter 约束 |

---

## TDD 计划

按 subagent-driven development 流程：

1. **Phase 1 测试**（reasoning_params.rs）：
   - `test_linear_mapping_boundaries` — 验证 trust=0.0→min, trust=1.0→max
   - `test_step_mapping_levels` — 验证每个主权级别的参数跳变
   - `test_conservative_mapping_ceiling` — 验证最高温度不超过 0.7
   - `test_reasoning_mapper_caching` — 验证相同 score 返回缓存
   - `test_strategy_swap` — 验证运行时切换策略

2. **Phase 2 测试**（reasoning_engine.rs）：
   - `test_diversity_score_range` — 验证 diversity ∈ [0, 1]
   - `test_response_deterministic_with_seed` — 验证同 seed 同参数 → 同输出
   - `test_high_temperature_produces_variety` — 验证高 temp → 更多样

3. **Phase 3 测试**（sovereign_agent_impl 集成）：
   - `test_reasoning_config_updates_with_trust` — 验证 trust 变化后 config 刷新
   - `test_user_space_can_read_config` — 验证 User Space 可读 config

4. **Phase 4 验证代理**：
   - 运行 `cargo run -p experiments --bin sovereignty_llm_experiment`
   - 手动检查输出的 5 条验证标准

---

## 实施顺序

1. `reasoning_params.rs` + 单元测试 → 确认参数映射正确
2. `reasoning_engine.rs` + 单元测试 → 确认模拟推理正确
3. 修改 `mod.rs` + `lib.rs` 导出 → 确认编译通过
4. 修改 `sovereign_agent_impl.rs` → 集成 ReasoningMapper
5. 创建 `sovereignty_llm_experiment.rs` → 完整实验
6. 修改 `Cargo.toml` → 注册新 bin
7. 运行验证 → 确认 5 条标准全部通过

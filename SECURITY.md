# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| main branch | ✅ |
| feat/* branches | ❌ |

## Reporting a Vulnerability

请通过 [安全问题反馈表单](https://github.com/Frostsnow35/Chora/issues/new?template=SECURITY.md) 提交问题。  
我们通常在24小时内响应。

## Disclosure Policy

1. **初始沟通**：安全研究人员应提供：
   - 漏细漏洞描述
   - 影响等级评估（基于 [CVSS v3.1](https://www.first.org/cvss/v3.1/specification-document)）
   - 最小化复现步骤

2. **修复流程**：
   ```mermaid
   graph LR
      A[安全问题提交] --> B{问题分类}
      B -->|高风险| C[3天内修复]
      B -->|中风险| D[10天内修复]
      B -->|低风险| E[下一版本修复]
      C --> F[安全公告]
      D --> F
      E --> F
   ```

3. **公告机制**：修复完成后将通过 GitHub Security Advisory 发布详细公告

## Security Best Practices

- **主權系统边界**：Kernel 空态类型不可被外部直接修改
- **信任度量器**：TrustMeter 分数计算需通过 SovereigntyGate 访问
- **能力通道**：所有工具调用必须经过 StepContext.capabilities 检查

## Audit Trail

| 审计点 | 位置 | 頻次 |
|--------|------|------|
| 主權等級變更 | runtime/src/sovereignty/gate.rs | 每次權限檢查 |
| 意圖修改提案 | runtime/src/sovereignty/intent_core.rs | 每次ProposeAmendment |
| 跨代理協商 | runtime/src/record.rs | 每次Channel通訊 |

> 请遵循 [RFC-003](rfc/003-sovereignty-system.md) 主權系统设计原则进行安全评估
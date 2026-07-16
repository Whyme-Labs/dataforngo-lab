# DataForNGO Lab — 参赛提交文案（中文）

> 用于填写 https://infinisynapse.cn/contest/vibe-coding/register 的中文内容。

## 应用名称
DataForNGO Lab — Insight Engine

## 应用简介 + 使用场景

**应用简介：**
DataForNGO Lab 是一个「自我进化」的运营智能引擎，帮助 NGO（公益组织）项目官员诊断受助项目「为何」表现不佳、以及「该怎么做」——并通过一道 PDPA 合规治理闸门，拦截任何可能重新识别到个人的发布行为。

**使用场景（明确使用场景）：**
面向公益 / 社会项目的影响力分析。项目官员选择一个受助人群分群（例如「25–40 岁 | 食品援助」），用自然语言提问（「为什么我们的完成率下降了？」），引擎返回一张嵌套推理图洞察卡：诊断（实际 vs 预期完成率）、一次模拟杠杆、以及一条由人工把关的建议。在发布之前，一道 GOVERN 治理闸门会强制校验 k-匿名（分群人数 ≥ k）、同意义务目的限制、以及 PII（个人身份信息）脱敏，并导出一张 PDPA 安全、可审计追溯的卡片。经过批准的建议会演进一套「跨租户学习手册」——即持久、可复用的核心资产。

**为何契合「泛数据分析」：**
它把杂乱的项目数据 + 一个自然语言问题，转化为结构化、受治理、可直接用于决策的洞察——正是 InfiniSynapse 所支撑的「数据 → 查询 → 洞察」闭环，平台负责提供研究 / 基准参照层。

## 作品地址
https://dataforngo-lab.swmengappdev.workers.dev

## InfiniSynapse API 集成说明

**架构（为何拆分本地与平台）：**
- **本地引擎**（Cloudflare Worker + Rust→WASM）负责所有严谨的数学计算：诊断、蒙特卡洛模拟、留出验证、估值。这是我们的护城河，必须保持可审计、无 PII。
- **InfiniSynapse** 提供「外部分析 / 研究层」：行业基准参照，以及为非技术项目官员提供的自然语言叙述。该调用仅通过服务端发起（绝不在浏览器端），并且**始终位于 GOVERN PII 闸门之后**（没有任何个人数据离开引擎）。

**当前接线（已于 2026-07-16 针对真实 Server API 实时验证）：**
- Worker 中的 `/api/narrate` 路由 → `worker/src/infini.ts` →
  `POST https://app.infinisynapse.cn/api/ai/message`（Bearer `sk-xxxx`，取自 `INFINI_API_KEY` secret），请求体为 `{type:"newTask", text, images:[], files:[], taskId, connId}`，随后轮询 `GET /api/ai_task/tasks?taskId=…` 并提取最终答案。
- **鉴权**：Bearer `sk-xxxx`，取自 `env.INFINI_API_KEY`（secret，通过 `wrangler secret put` 设置）。该调用返回 `createdVia:"api_key"`——即它会被记入 InfiniSynapse 后端该 API key 的调用日志（评委可核验的信号）。
- `data_source` 为**可选**：当设置了 `env.INFINI_DATA_SOURCE`（普通变量）时，会作为 `databaseIds` 附带；即便不设置，叙述功能仍可正常工作，因为提示词被构建为「叙述以下已提供的结果」。
- 每次调用前都会执行 `scanPii`；若发现 PII，则不会发送任何数据（GOVERN 闸门生效）。

**说明：** 大赛公告中的 `/v1/query` 示例在生产环境返回 404；上方经核实的端点是实时控制台实际使用的、并能够产生 API key 调用日志的端点。API key `sk-6a57…ad6d` 已于 2026-07-16 在控制台 UI 中创建并设为 secret。

## 代码仓库
本工作区 `contest-app/` 目录（Cloudflare Worker + Rust→WASM 源码）。

## 使用截图
- `contest-app/screenshot-allow.png`（允许发布：洞察卡生成）
- `contest-app/screenshot-kanon-block.png`（k-匿名拦截）
- `contest-app/screenshot-consent-block.png`（同意义务拦截）

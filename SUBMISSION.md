# DataForNGO Lab — InfiniSynapse Vibe Coding Contest Submission

> Status: **SUBMITTED (2026-07-16).** The InfiniSynapse API Key was created via the console UI
> and set as a `wrangler` secret; the integration is realigned to the **verified** Server
> API (`app.infinisynapse.cn/api/ai/message` + poll) and **verified live** (2026-07-16):
> insight → "Generate narration (Infini)" → clean SEA-benchmarked briefing, logged as an
> API-key call. Public URL is deployed. Public repo created under Whyme-Labs and pushed.
> Form submitted by owner. Remaining: driving user acquisition for the 60%-weight
> user-metric score (§5).

## 1. Submission form fields (报名入口: https://infinisynapse.cn/contest/vibe-coding/register)

| Field | Value |
|---|---|
| 应用名称 (App name) | **DataForNGO Lab — Insight Engine** |
| 应用简介 + 使用场景 (Intro + use scenario) | See §2 |
| 作品地址 (Public URL) | **https://dataforngo-lab.swmengappdev.workers.dev** |
| InfiniSynapse API 集成说明 (Integration description) | See §3 |
| 代码仓库 (Repo) | `https://github.com/Whyme-Labs/dataforngo-lab` |
| 使用截图 (Screenshots) | `contest-app/screenshot-allow.png`, `screenshot-kanon-block.png`, `screenshot-consent-block.png` |

## 2. App intro + use scenario (English; contest is CN — provide ZH version at submit)

**One-liner:** A self-evolving operations-intelligence engine that helps NGO program
officers diagnose *why* a beneficiary program is underperforming and *what* to do — with
a PDPA-safe governance gate that blocks any publish that could re-identify a person.

**Use scenario (明确使用场景):** Non-profit / social-program impact analysis. A program
officer picks a beneficiary segment (e.g. "25–40 | Food Aid"), asks a plain-language
question ("why is our completion rate dropping?"), and the engine returns a nested
reasoning-graph insight: a diagnosis (observed vs expected completion), a simulated
lever, and a human-gated recommendation. Before anything is published, a **GOVERN gate**
enforces k-anonymity (segment size ≥ k), consent-purpose limitation, and PII redaction,
and exports a PDPA-safe, audit-traced card. Approved recommendations evolve a
**cross-tenant learned playbook** — the durable, reusable asset.

**Why it fits "泛数据分析 (pan-data-analysis)":** it turns messy program data + a natural
question into a structured, governed, decision-ready analysis — exactly the
"data → query → insight" loop InfiniSynapse backs, with the platform supplying the
research/benchmark layer.

## 3. InfiniSynapse API integration description

**Architecture (why we split local vs platform):**
- The **local engine** (Cloudflare Worker + Rust→WASM) owns all rigorous math: diagnosis,
  Monte-Carlo simulation, holdout validation, valuation. This is the moat and must stay
  auditable and PII-free.
- **InfiniSynapse** supplies the *external analysis / research layer*: sector benchmarks
  and plain-language narration for a non-technical officer, called server-side only, never
  from the browser, and **always behind the GOVERN PII gate** (no personal data leaves the
  engine).

**Current wiring (verified live 2026-07-16 against the real Server API):**
- Endpoint `/api/narrate` in the Worker → `worker/src/infini.ts` →
  `POST https://app.infinisynapse.cn/api/ai/message` (Bearer `sk-xxxx` from the
  `INFINI_API_KEY` secret) with body `{type:"newTask", text, images:[], files:[], taskId, connId}`,
  then polls `GET /api/ai_task/tasks?taskId=…` and extracts the final answer.
- Authentication: Bearer `sk-xxxx` from `env.INFINI_API_KEY` (secret, set with
  `wrangler secret put`). The call returns `createdVia:"api_key"` — i.e. it is logged
  against the API key in the InfiniSynapse backend (the judge-verifiable signal).
- `data_source` is **optional**: `env.INFINI_DATA_SOURCE` (plain var) is attached as
  `databaseIds` when set; narration works without it because the prompt is framed as
  "narrate these supplied findings."
- Every call is preceded by `scanPii`; if PII is found, nothing is sent (GOVERN gate).

> Note: the contest announcement's `/v1/query` example 404s in production; the verified
> endpoint above is what the live console actually uses and what produces the API-key log.
> API key `sk-6a57…ad6d` was created in the console UI and set as the secret on 2026-07-16.

## 4. Verified behaviour (real UI click-through, 2026-07-15)

All paths tested live at the public URL via real Chrome:
1. **Allow** — Food Aid (n=80 ≥ k=5): insight card generated, playbook v1.
2. **k-anonymity block** — Pilot Micro (n=3 < k=5): *Publish blocked — re-identification risk*.
3. **Consent block** — Revoke consent: *NONE supplied — purpose-limitation failed*.
4. **PII scan** — pasted name + NRIC + email: *Blocked — 2 PII match(es): email, ic_or_id*.
5. **Approve → evolve** — playbook advances to v2 (versioned + audited in a Durable Object).
6. **Reset → v1** — playbook restored to v1.
7. **PDPA-safe export** — downloads a clean `redacted_card` (no PII) + full audit trail.

## 5. Pre-submission checklist

**Done (2026-07-16):**
- [x] **Register / log in** at https://app.infinisynapse.cn (`.cn` console session active).
- [x] **Create API Key** `sk-6a57…ad6d` in the `.cn` console (`/ai/apikey`) — done via browser.
- [x] `wrangler secret put INFINI_API_KEY` set; deployed; **live call verified** (returns `createdVia:"api_key"`).
- [x] Public URL live: https://dataforngo-lab.swmengappdev.workers.dev (insight + "Generate narration (Infini)" → briefing verified in real Chrome).
- [x] **Public GitHub repo created + pushed**: https://github.com/Whyme-Labs/dataforngo-lab (MIT, README, .gitignore, .dev.vars.example; no secrets).
- [x] **Submit** at https://infinisynapse.cn/contest/vibe-coding/register — completed by owner (2026-07-16).

**Optional polish:**
- [ ] (Optional) Create a `data_source` from the NGO CSV and set `INFINI_DATA_SOURCE` for
      richer grounding — narration already works without it.
- [ ] (Optional, for the 60% user-metric score) Integrate **Partner SSO「InfiniSynapse 登录」**
      so visitors who authorize become attributed registered+active users. Guide:
      https://infinisynapse.cn/zh/docs/InfiniSynapse%20Partner%20SSO%20Integration%20Guide

**Scoring — 60% user metrics (registered + active usage on InfiniSynapse backend):**
- [ ] Drive real users: share URL on LinkedIn/X; post in the contest 微信活动群 (互评 =
      mutual usage boosts metrics); NGO/SEA tech communities in KL.

**Scoring — 40% expert (scenario value, technical completeness, innovation):**
- [ ] Keep the 3 screenshots + add a short demo video (optional but strengthens "technical completeness").
- [ ] Highlight differentiators: self-evolving cross-tenant playbook, PDPA-safe GOVERN gate, auditable loop.

**Reference docs (read before coding the alignment):**
- Server API Reference: https://infinisynapse.cn/zh/docs/InfiniSynapse%20Server%20API%20Reference
- Vibe Coding Guide: https://www.infinisynapse.cn/zh/docs/InfiniSynapse%20Vibe%20Coding%20Guide
- Sample app: https://github.com/chaozwn/infini_app

## 6. Timeline

- Deadline: **2026-07-31 23:59**. Review 8/1–8/11.
- We are contest-ready on the *product*; the only hard external dependency is your API
  Key + data source. Once you provide them, alignment + redeploy + verify is ~1 focused pass.

# DataForNGO Lab — Insight Engine

> A self-evolving operations-intelligence engine for NGOs. Picks a beneficiary
> segment, answers a plain-language "why is this underperforming / what should we
> do?" question, and returns a governed, decision-ready insight — with a PDPA-safe
> gate that blocks any publish that could re-identify a person.
>
> Built for the **InfiniSynapse × CSDN "Vibe Coding" Pan-Data Analysis App Dev Contest**.

**Live demo:** https://dataforngo-lab.swmengappdev.workers.dev

---

## Why this exists

Non-profit / social-program teams rarely have data scientists, yet they sit on
messy program data and face hard questions: *why is our completion rate dropping?*
*Which lever actually moves outcomes?* And they must answer those questions without
breaking PDPA / data-protection obligations.

DataForNGO Lab turns that into a structured loop:

```
ingest → diagnose → propose → validate → approve
            (skills evolve, model frozen, audit lineage kept)
```

The output is a **nested reasoning-graph insight card**: a diagnosis (observed vs
expected), a simulated lever, and a human-gated recommendation. Before anything is
published, a **GOVERN gate** enforces k-anonymity, consent / purpose limitation,
and PII redaction, then exports a PDPA-safe, audit-traced card. Approved
recommendations evolve a **cross-tenant learned playbook** — the durable, reusable
asset that gets stronger the more organisations use it.

## Features

- **Nested insight graph** — diagnosis → simulation → recommendation in one card.
- **GOVERN gate (PDPA-safe publishing)** — blocks publish when:
  - segment size < k (k-anonymity, default k=5) → re-identification risk,
  - consent is missing / revoked → purpose-limitation failed,
  - PII detected in input → redaction required.
  Exports a clean `redacted_card` + full audit trail.
- **Self-evolving playbook** — versioned + audited in a Durable Object
  (`PlaybookStore`, SQLite-backed, free-plan friendly). Reset restores v1.
- **InfiniSynapse integration** — server-side calls to InfiniSynapse supply the
  external research / benchmark / narration layer (see below).

## Architecture

```
┌─────────────────────────┐
│  public/index.html      │  static UI (served as Worker Assets)
└───────────┬─────────────┘
            │ fetch
┌───────────▼─────────────┐
│  Cloudflare Worker (TS) │  routing + GOVERN gate + playbook DO
│   ├─ /api/insight       │  build nested insight graph
│   ├─ /api/narrate       │  → InfiniSynapse (server-side only)
│   └─ /api/playbook      │  get / approve / reset
└───────────┬─────────────┘
   ┌─────────┴──────────┐
┌──▼───┐          ┌──────▼──────┐
│ WASM │ engine_core  (Rust→WASM): diagnosis, Monte-Carlo
│ core │ simulation, holdout validation, valuation — the moat,
└──────┘ auditable & PII-free
┌──────────────┐
│ PlaybookStore│ Durable Object (SQLite): cross-tenant learned playbook
└──────────────┘
```

- **Local engine** owns all rigorous math (Rust→WASM). It must stay auditable and
  PII-free.
- **InfiniSynapse** is the *external analysis / research layer*: sector benchmarks
  and plain-language narration for non-technical officers. Called **server-side
  only** (never from the browser) and **always behind the GOVERN PII gate** — no
  personal data leaves the engine.
- **Heavy backend (optional)** — `heavy-backend/` is a Python service for causal/ML
  estimates; off by default (`HEAVY_BACKEND_URL` empty → in-edge WASM estimates).

### InfiniSynapse API integration (verified)

`worker/src/infini.ts` → `POST https://app.infinisynapse.cn/api/ai/message`
(`{type:"newTask", text, images:[], files:[], taskId, connId}`, Bearer `sk-xxxx`
from the `INFINI_API_KEY` secret) → poll
`GET /api/ai_task/tasks?taskId=…` → extract final answer. The call returns
`createdVia:"api_key"`, i.e. it is logged against the API key in the InfiniSynapse
backend (judge-verifiable). `data_source` is optional. Every call is preceded by a
PII scan; if PII is found, nothing is sent.

> Note: the contest announcement's `/v1/query` example 404s in production; the
> endpoint above is what the live console actually uses.

## Project layout

```
contest-app/
├── worker/          # Cloudflare Worker (TS): routing, GOVERN gate, playbook
├── engine-core/     # Rust → WASM core (diagnosis, simulation, validation)
├── heavy-backend/   # optional Python causal/ML backend
├── skills/          # YouthSkillsImpact skill (first vertical)
├── playbook/        # learned-playbook store + seed
├── public/          # static UI (index.html)
├── docs/            # submission notes (SUBMISSION.md / SUBMISSION_ZH.md)
├── wrangler.toml
└── package.json
```

## Deploy

Requires: Rust (`wasm32-unknown-unknown` target), Node, `wrangler`, and an
InfiniSynapse API key.

```bash
# 1. install deps
npm install

# 2. build the WASM core (or commit worker/engine_core.wasm)
npm run build:wasm

# 3. set the InfiniSynapse API key as a secret
wrangler secret put INFINI_API_KEY

# 4. (optional) point at a registered data_source id
wrangler variable put INFINI_DATA_SOURCE "<your-datasource-id>"

# 5. deploy
npm run deploy
```

Local dev: `npm run dev` (uses `.dev.vars` — see `.dev.vars.example`).

## License

MIT — see [LICENSE](LICENSE).

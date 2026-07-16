// Worker env bindings. The Rust->WASM core is imported directly as a module in
// worker/src/engine.ts (ADR-0004b) — no runtime binding needed here.
export interface Env {
  ASSETS: { fetch: (req: Request) => Promise<Response> };
  PLAYBOOK: DurableObjectNamespace;
  INFINI_API_KEY?: string;
  INFINI_BASE_URL?: string;
  INFINI_DATA_SOURCE?: string;
  // Heavy Python model backend (ADR-0018). Optional: when set, /api/insight
  // dispatches to it for data-driven causal estimates; otherwise in-edge only.
  HEAVY_BACKEND_URL?: string;
  HEAVY_BACKEND_TOKEN?: string;
}

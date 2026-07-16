import { runInsight, scanPii } from "./engine";
import { narrate } from "./infini";
import { SKILL_MANIFEST, SKILL_STATE, PLAYBOOK, SAMPLE, CONSENT } from "./config";
import { PlaybookStore, type PlaybookState, type ApprovedEntry } from "./playbook_store";

export { PlaybookStore };

interface Env {
  ASSETS: { fetch: (req: Request) => Promise<Response> };
  PLAYBOOK: DurableObjectNamespace;
  INFINI_API_KEY?: string;
  INFINI_BASE_URL?: string;
  INFINI_DATA_SOURCE?: string;
  // Heavy Python model backend (ADR-0018). Optional.
  HEAVY_BACKEND_URL?: string;
  HEAVY_BACKEND_TOKEN?: string;
}

interface InsightCard {
  segment: string;
  recommended_lever: number;
  expected_completion: number;
  projected_completion: number;
  additional_completions: number;
  holdout_mae: number;
  validated: boolean;
  approval: string;
  lineage: { run_id: string };
  [k: string]: unknown;
}

// Single authoritative instance of the learned playbook.
function store(env: Env): DurableObjectStub {
  return env.PLAYBOOK.get(env.PLAYBOOK.idFromName("global"));
}

async function playbookState(env: Env): Promise<PlaybookState> {
  const res = await store(env).fetch("https://do/state");
  return (await res.json()) as PlaybookState;
}

// Merge the seed (pre-loaded) playbook with the evolved DO state so the engine
// sees both illustrative baselines and human-approved entries.
function mergedPlaybook(state: PlaybookState) {
  return { ...PLAYBOOK, version: state.version, approved_entries: state.entries };
}

// Run the in-edge WASM engine, then optionally overlay data-driven estimates
// from the heavy Python backend (ADR-0018). Falls back silently to in-edge
// values when the backend is unreachable, or when the engine found no
// actionable gap for this segment (so GOVERN on-track semantics are preserved).
async function runInsightAndMaybeRefine(env: Env, queryJson: string): Promise<any> {
  const card: any = await runInsight(queryJson);
  if (!env.HEAVY_BACKEND_URL || !(card.recommended_lever > 0)) {
    card.meta = {
      ...(card.meta || {}),
      heavy_backend: env.HEAVY_BACKEND_URL ? "in-edge" : "in-edge (no heavy backend configured)",
    };
    return card;
  }
  try {
    const q = JSON.parse(queryJson);
    const seg = String(q.segment).split("|")[1];
    const targetN = ((q.programs as any[]) || []).find((p: any) => p.program_name === seg)?.target_n ?? 0;
    const res = await fetch(`${env.HEAVY_BACKEND_URL}/refine`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        ...(env.HEAVY_BACKEND_TOKEN ? { authorization: `Bearer ${env.HEAVY_BACKEND_TOKEN}` } : {}),
      },
      body: JSON.stringify({ segment: q.segment, beneficiaries: q.beneficiaries, card, target_n: targetN }),
    });
    if (res.ok) {
      const o: any = await res.json();
      card.projected_completion = o.projected_completion;
      card.additional_completions = o.additional_completions;
      card.recommended_lever = o.recommended_lever;
      card.meta = { ...(card.meta || {}), heavy_backend: o.heavy_backend, ...(o.model_versions || {}) };
      if (o.node_narratives) {
        for (const n of card.children || []) {
          const nn = o.node_narratives[n.id];
          if (nn) n.narrative = nn;
        }
      }
      return card;
    }
  } catch (_e) {
    /* fall through to in-edge */
  }
  card.meta = { ...(card.meta || {}), heavy_backend: "in-edge (heavy backend unreachable)" };
  return card;
}

function buildQuery(
  body: { question?: string; segment?: string; beneficiaries?: unknown[]; programs?: unknown[]; consent?: unknown },
  state: PlaybookState
) {
  return {
    question: body.question ?? "Why is this segment underperforming and what should we change?",
    segment: body.segment ?? "25-40|Food Aid",
    beneficiaries: body.beneficiaries ?? SAMPLE.beneficiaries,
    programs: body.programs ?? SAMPLE.programs,
    skill_manifest: SKILL_MANIFEST,
    skill_state: SKILL_STATE,
    playbook: mergedPlaybook(state),
    // Consent defaults to the seed manifest; the client may send null to
    // simulate a withdrawn/never-granted consent (GOVERN publish block).
    consent: body.consent !== undefined ? body.consent : CONSENT,
  };
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);

    if (url.pathname === "/api/insight" && req.method === "POST") {
      const body = (await req.json()) as Parameters<typeof buildQuery>[0];
      const state = await playbookState(env);
      try {
        const card = await runInsightAndMaybeRefine(env, JSON.stringify(buildQuery(body, state)));
        return Response.json(card);
      } catch (e) {
        return Response.json({ error: String(e) }, { status: 500 });
      }
    }

    // Human approval gate → evolve the learned playbook (ADR-0012, the moat).
    if (url.pathname === "/api/approve" && req.method === "POST") {
      const body = (await req.json()) as Parameters<typeof buildQuery>[0] & { approved_by?: string };
      const state = await playbookState(env);
      try {
        const card = (await runInsight(JSON.stringify(buildQuery(body, state)))) as InsightCard;
        if (!(card.recommended_lever > 0)) {
          return Response.json(
            { error: "No actionable recommendation to approve for this segment." },
            { status: 400 }
          );
        }
        const entry: ApprovedEntry = {
          segment: card.segment,
          recommended_lever: card.recommended_lever,
          expected_completion: card.expected_completion,
          projected_completion: card.projected_completion,
          additional_completions: card.additional_completions,
          approved_at: new Date().toISOString(),
          approved_by: body.approved_by ?? "demo-user",
          run_id: card.lineage.run_id,
          holdout_mae: card.holdout_mae,
          playbook_version: 0, // set by the store on evolve
        };
        const evolveRes = await store(env).fetch("https://do/evolve", {
          method: "POST",
          body: JSON.stringify(entry),
        });
        const newState = (await evolveRes.json()) as PlaybookState;
        // Re-run so the returned card reflects the just-evolved playbook.
        const validatedCard = await runInsightAndMaybeRefine(env, JSON.stringify(buildQuery(body, newState)));
        return Response.json({ state: newState, card: validatedCard });
      } catch (e) {
        return Response.json({ error: String(e) }, { status: 500 });
      }
    }

    if (url.pathname === "/api/playbook") {
      return Response.json(await playbookState(env));
    }

    if (url.pathname === "/api/reset" && req.method === "POST") {
      const res = await store(env).fetch("https://do/reset", { method: "POST" });
      return Response.json((await res.json()) as PlaybookState);
    }

    if (url.pathname === "/api/scan" && req.method === "POST") {
      const { text } = (await req.json()) as { text?: string };
      // GOVERN gate (ADR-0011/0012): scan before any external (Infini) call.
      const findings = (await scanPii(text ?? "")) as unknown[];
      return Response.json({ blocked: findings.length > 0, findings });
    }

    // InfiniSynapse Server API: skill-bounded research + narration (ADR-0002/0008).
    // The GOVERN PII gate runs inside narrate() before any external call.
    if (url.pathname === "/api/narrate" && req.method === "POST") {
      const { card } = (await req.json()) as { card?: unknown };
      if (!card) return Response.json({ error: "missing card" }, { status: 400 });
      const result = await narrate(card, env.INFINI_API_KEY, env.INFINI_BASE_URL ?? "https://app.infinisynapse.cn", env.INFINI_DATA_SOURCE);
      const status = result.pii_blocked ? 402 : result.error && !result.disabled ? 502 : 200;
      return Response.json(result, { status });
    }

    // Static frontend (ADR-0007).
    return env.ASSETS.fetch(req);
  },
};

// Server-side InfiniSynapse integration (ADR-0002 / ADR-0008 / ADR-0016).
// Infini does RESEARCH + NARRATION only, inside the skill contract. The local
// engine owns all math (ADR-0005). Every external call is preceded by the
// GOVERN PII gate (scanPii) so no personal data is ever sent to a 3rd-party LLM.
//
// VERIFIED contest Server API (discovered by capturing the live console call):
//   1. POST https://app.infinisynapse.cn/api/ai/message
//        headers: Authorization: Bearer <sk-...>
//        body:   { type:"newTask", text, images:[], files:[], taskId, connId }
//      -> returns { data.state.currentTaskItem.id, ... , createdVia:"api_key" }
//         (call is logged against the API key — judge-verifiable)
//   2. GET  https://app.infinisynapse.cn/api/ai_task/tasks?taskId=<id>
//      -> { data.taskInfo.status, data.messages[] } ; poll until status==="completed"
//      -> final answer = last message with say==="text" whose text != prompt
import { scanPii } from "./engine";

export interface InfiniResult {
  narration?: string;
  pii_blocked?: boolean;
  findings?: unknown[];
  error?: string;
  disabled?: boolean;
}

const pct = (v: number) => `${(v * 100).toFixed(0)}%`;

// Build a skill-bounded prompt: research + narration, engine numbers frozen in.
// Framed as "narrate these supplied findings" so the model never asks for data.
export function buildPrompt(card: any): string {
  const seg = card?.segment ?? "unknown segment";
  const question = card?.question ?? "";
  const nodes: any[] = card?.children ?? [];
  const find = (id: string) => nodes.find((n: any) => n.id === id);
  const diag = find("n-diagnose");
  const prop = find("n-propose");
  const ev = find("n-evaluate");
  const diagTxt = diag
    ? `${pct(diag.value)} (95% CI ${pct(diag.ci?.[0] ?? diag.value)}–${pct(diag.ci?.[1] ?? diag.value)})`
    : "n/a";
  const lever = prop?.value ? `${(prop.value * 100).toFixed(0)}pp` : "n/a";
  const extra = ev?.value != null ? `${Math.round(ev.value)}` : "n/a";
  return [
    "Write a short plain-language briefing (maximum 110 words) for a non-technical NGO program officer.",
    "Use ONLY the figures below — never invent or alter a number. Do not ask for data or a data source.",
    "Output the briefing text ONLY: no headings, no 'Task 1/2' labels, no planning notes, no meta-commentary.",
    "Structure: one sentence of external SEA social-program completion benchmark context, then 2–3 sentences explaining what the numbers mean and whether the recommended action is warranted.",
    "",
    "Supplied findings:",
    `- Segment: ${seg}`,
    question ? `- Program officer's question: ${question}` : "",
    `- Current completion: ${diagTxt}`,
    `- Recommended lever: ${lever} (if 'n/a', the segment is on track and needs no action)`,
    `- Estimated additional completions from the lever: ${extra}`,
  ].filter(Boolean).join("\n");
}

// Extract the final answer from the polled task: last say==="text" message
// whose text differs from the prompt (so we skip the user's own question echo).
function extractAnswer(json: any, prompt: string): string {
  const msgs: any[] = json?.data?.messages ?? [];
  const picked = msgs.filter(
    (m) => m?.say === "text" && typeof m.text === "string" && m.text.trim() !== prompt.trim(),
  );
  if (picked.length) return picked[picked.length - 1].text.trim();
  // fall back to any text payload
  const any = msgs.filter((m) => typeof m?.text === "string" && m.text.trim());
  if (any.length) return any[any.length - 1].text.trim();
  return "";
}

async function uuid(): Promise<string> {
  // Cloudflare Workers (and modern browsers) provide crypto.randomUUID().
  return (crypto as any).randomUUID();
}

export async function narrate(
  card: any,
  apiKey: string | undefined,
  baseUrl: string,
  dataSource?: string,
): Promise<InfiniResult> {
  if (!apiKey) {
    return { disabled: true, error: "LLM narration disabled — set INFINI_API_KEY secret." };
  }

  const prompt = buildPrompt(card);

  // GOVERN gate: never send PII to a third-party LLM.
  const findings = (await scanPii(prompt)) as unknown[];
  if (Array.isArray(findings) && findings.length > 0) {
    return { pii_blocked: true, findings };
  }

  const base = baseUrl.replace(/\/$/, "");
  const taskId = await uuid();
  const connId = await uuid();

  // Step 1: start the task.
  const start: any = await fetch(`${base}/api/ai/message`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      type: "newTask",
      text: prompt,
      images: [],
      files: [],
      taskId,
      connId,
      ...(dataSource ? { databaseIds: [dataSource] } : {}),
    }),
  }).then((r) => r.json().catch(() => ({} as any)));

  if (start?.code && start.code >= 400) {
    return { error: `Infini start failed: HTTP ${start.code}. ${JSON.stringify(start).slice(0, 300)}` };
  }
  const realTaskId: string = start?.data?.state?.currentTaskItem?.id ?? taskId;

  // Step 2: poll for completion (bounded; Workers have a 30s wall-clock limit).
  let answer = "";
  for (let i = 0; i < 7; i++) {
    await new Promise((res) => setTimeout(res, 3000));
    const poll: any = await fetch(`${base}/api/ai_task/tasks?taskId=${realTaskId}`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    }).then((r) => r.json().catch(() => ({} as any)));
    const status: string = poll?.data?.taskInfo?.status ?? "";
    answer = extractAnswer(poll, prompt);
    if (status === "completed" && answer) break;
    // if "waiting" (awaiting input) but we already have a narrative, take it.
    if (status === "waiting" && answer && !/请提供|需要您|请上传/.test(answer)) break;
  }

  if (!answer) return { error: "Infini returned no answer in time." };
  return { narration: answer };
}

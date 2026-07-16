// PlaybookStore: the persistent, versioned, audited home of the cross-tenant
// learned playbook (the moat, ADR-0012). A single Durable Object instance
// ("global") serialises all read-modify-write, so the append-only audit log is
// race-free and every evolution is traceable (GOVERN lineage). SQLite-backed so
// it runs on the Workers free plan.

export interface ApprovedEntry {
  segment: string;
  recommended_lever: number;
  expected_completion: number;
  projected_completion: number;
  additional_completions: number;
  approved_at: string;
  approved_by: string;
  run_id: string;
  holdout_mae: number;
  playbook_version: number;
}

export interface AuditEvent {
  version: number;
  segment: string;
  recommended_lever: number;
  approved_at: string;
  approved_by: string;
  run_id: string;
  holdout_mae: number;
}

export interface PlaybookState {
  version: number;
  entries: Record<string, ApprovedEntry>;
  audit: AuditEvent[];
}

export class PlaybookStore {
  private state: DurableObjectState;

  constructor(state: DurableObjectState) {
    this.state = state;
  }

  private async read(): Promise<PlaybookState> {
    const version = (await this.state.storage.get<number>("version")) ?? 1;
    const entries = (await this.state.storage.get<Record<string, ApprovedEntry>>("entries")) ?? {};
    const audit = (await this.state.storage.get<AuditEvent[]>("audit")) ?? [];
    return { version, entries, audit };
  }

  async fetch(req: Request): Promise<Response> {
    const url = new URL(req.url);

    if (url.pathname === "/state") {
      return Response.json(await this.read());
    }

    if (url.pathname === "/evolve" && req.method === "POST") {
      const entry = (await req.json()) as ApprovedEntry;
      const cur = await this.read();
      const version = cur.version + 1;
      entry.playbook_version = version;
      const entries = { ...cur.entries, [entry.segment]: entry };
      const audit = [
        ...cur.audit,
        {
          version,
          segment: entry.segment,
          recommended_lever: entry.recommended_lever,
          approved_at: entry.approved_at,
          approved_by: entry.approved_by,
          run_id: entry.run_id,
          holdout_mae: entry.holdout_mae,
        },
      ];
      await this.state.storage.put("version", version);
      await this.state.storage.put("entries", entries);
      await this.state.storage.put("audit", audit);
      return Response.json({ version, entries, audit } as PlaybookState);
    }

    if (url.pathname === "/reset" && req.method === "POST") {
      await this.state.storage.put("version", 1);
      await this.state.storage.put("entries", {});
      await this.state.storage.put("audit", []);
      return Response.json({ version: 1, entries: {}, audit: [] } as PlaybookState);
    }

    return new Response("not found", { status: 404 });
  }
}

# DataForNGO Lab — social post / article draft

Title: I built a self-evolving, PDPA-safe ops-intelligence engine for NGOs

Paragraphs (each a single line; safe for ProseMirror / Draft.js injection):

1. Most NGOs sit on messy program data and face hard questions — why is our completion rate dropping, which lever actually moves outcomes — without a data team to answer them. So I built DataForNGO Lab: an operations-intelligence engine that turns a plain-language question into a governed, decision-ready insight.

2. The core is a self-evolving loop: ingest → diagnose → propose → validate → approve. Ask "why is our completion rate dropping?" and you get a nested reasoning-graph card — a diagnosis (observed vs expected), a simulated lever, and a human-gated recommendation.

3. The part I care about is durability and compliance. Before anything is published, a GOVERN gate enforces k-anonymity (segment size >= k), consent and purpose-limitation, and PII redaction — then exports a PDPA-safe, audit-traced card. No publish that could re-identify a person gets through.

4. Approved recommendations evolve a cross-tenant learned playbook — the durable asset. Skills improve, the model stays frozen, and every step keeps its audit lineage. The engine gets stronger the more organisations use it, without ever shipping personal data.

5. Under the hood it is a Cloudflare Worker (TypeScript) plus a Rust→WASM core for the rigorous math — diagnosis, Monte-Carlo simulation, holdout validation. A Durable Object stores the playbook. InfiniSynapse supplies the external research and narration layer, called server-side only and always behind the PII gate.

6. It is live and open source. Try it here: https://dataforngo-lab.swmengappdev.workers.dev — and the code is at https://github.com/Whyme-Labs/dataforngo-lab (MIT).

7. This is my entry for the InfiniSynapse × CSDN Vibe Coding contest. If you work in NGO, social impact, or SEA tech and want a compliant way to turn program data into decisions, give it a spin and tell me what breaks.

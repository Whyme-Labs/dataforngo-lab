// Thin TS edge over the Rust->WASM core (ADR-0004b). The core exposes a plain
// C ABI (no wasm-bindgen) to avoid the function-table growth failure that
// wasm-bindgen hits inside the Cloudflare Workers V8 runtime. We allocate a
// buffer, marshal the UTF-8 input, call the exported fn, read back a
// null-terminated C string, then free it.
import engine_wasm from "../engine_core.wasm";

type WasmExports = {
  memory: WebAssembly.Memory;
  alloc: (n: number) => number;
  dealloc: (ptr: number, n: number) => void;
  free_str: (ptr: number) => void;
  run_insight: (ptr: number, len: number) => number;
  scan_pii: (ptr: number, len: number) => number;
};

let instance: WebAssembly.Instance | null = null;

async function ensure(): Promise<WebAssembly.Instance> {
  if (instance) return instance;
  // esbuild/wrangler hands us a WebAssembly.Module for the .wasm import.
  instance = await WebAssembly.instantiate(engine_wasm as unknown as WebAssembly.Module);
  return instance;
}

function callString(fn: "run_insight" | "scan_pii", input: string): string {
  const inst = instance as WebAssembly.Instance;
  const ex = inst.exports as unknown as WasmExports;
  const enc = new TextEncoder();
  const bytes = enc.encode(input);

  const ptr = ex.alloc(bytes.length);
  new Uint8Array(ex.memory.buffer).set(bytes, ptr);
  const outPtr = ex[fn](ptr, bytes.length);

  // Read null-terminated C string from outPtr. Memory may have grown inside
  // the call, so re-fetch the buffer now.
  const mem = new Uint8Array(ex.memory.buffer);
  let end = outPtr;
  while (mem[end] !== 0) end++;
  const out = new TextDecoder().decode(mem.slice(outPtr, end));
  ex.free_str(outPtr);
  return out;
}

export async function runInsight(queryJson: string): Promise<unknown> {
  await ensure();
  return JSON.parse(callString("run_insight", queryJson));
}

export async function scanPii(text: string): Promise<unknown> {
  await ensure();
  return JSON.parse(callString("scan_pii", text));
}

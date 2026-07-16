// Global ambient declarations for non-TS assets resolved by the bundler.
declare module "*.wasm" {
  const module: WebAssembly.Module;
  export default module;
}

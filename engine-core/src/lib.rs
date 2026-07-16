mod types;
mod stats;
mod models;
mod engine;
mod govern;

use std::alloc::Layout;
use std::ffi::CString;

// Plain C ABI (no wasm-bindgen) to avoid the function-table growth issue that
// wasm-bindgen hits inside the Cloudflare Workers V8 runtime. The TS edge
// (worker/src/engine.ts) allocates, marshals strings, and frees.

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size.max(1), 1).unwrap();
    unsafe { std::alloc::alloc(layout) }
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    if ptr.is_null() {
        return;
    }
    let layout = Layout::from_size_align(size.max(1), 1).unwrap();
    unsafe { std::alloc::dealloc(ptr, layout) };
}

#[no_mangle]
pub extern "C" fn run_insight(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unsafe { std::slice::from_raw_parts(ptr, len) };
    let q = String::from_utf8_lossy(input);
    let out = match engine::run_insight(&q) {
        Ok(card) => serde_json::to_string(&card).unwrap_or_else(|_| "{}".to_string()),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    };
    CString::new(out).unwrap_or_default().into_raw() as *mut u8
}

#[no_mangle]
pub extern "C" fn scan_pii(ptr: *const u8, len: usize) -> *mut u8 {
    let input = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(input);
    let f = govern::scan_pii(&text);
    let out = serde_json::to_string(&f).unwrap_or_else(|_| "[]".to_string());
    CString::new(out).unwrap_or_default().into_raw() as *mut u8
}

#[no_mangle]
pub extern "C" fn free_str(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr as *mut i8)) };
}

use wasm_bindgen::prelude::*;

use wasm_streams::readable::*;

#[wasm_bindgen(module = "/tests/js/readable_stream.js")]
extern "C" {
    pub fn new_noop_readable_stream() -> sys::ReadableStream;
    pub fn new_readable_stream_from_array(chunks: Box<[JsValue]>) -> sys::ReadableStream;
}

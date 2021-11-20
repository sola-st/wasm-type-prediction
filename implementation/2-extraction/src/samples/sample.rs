use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct WasmTypeSample<WasmRepr, TypeRepr, Aux = ()> {
    // Metainformation, useful for debugging samples.
    // Use shared pointers for each individual datum, since:
    // 1) metainformation is shared (no need to clone the filename multiple times)
    // 2) the lifetime of different pieces of the metainformation is different, e.g., `file` is the
    // same across all samples from a single binary vs. compilation units vs. function.
    // Use atomic ref-counts because those samples are processed in parallel and rayon needs the 
    // type to be Send.
    pub file: Arc<Path>,
    pub compilation_unit: Option<Arc<str>>,

    pub function_idx: u32,
    pub function_name_wasm: Option<Arc<str>>,
    pub function_name_dwarf: Option<Arc<str>>,

    pub param_or_return: ParamOrReturn,

    pub wasm_type: wasmparser::Type,

    // The struct is generic over the WebAssembly and type representation: first it contains
    // the full body and the original DWARF type, later a prefix of the instructions and our own
    // type language.
    pub wasm_body: WasmRepr,
    pub type_: TypeRepr,

    // Auxiliary information that can be attached to this sample, e.g., dataset subset.
    pub aux: Aux,
}

#[derive(Debug, Clone)]
pub enum ParamOrReturn {
    Param {
        idx: u32,
        name: Option<Box<str>>,
    },
    Return
}

// Generic struct update methods, since Rust's update syntax doesn't work.
impl<T, U, V> WasmTypeSample<T, U, V> {
    pub fn map_wasm_body<R>(self, f: impl FnOnce(T) -> R) -> WasmTypeSample<R, U, V> {
        WasmTypeSample {
            file: self.file,
            compilation_unit: self.compilation_unit,
            function_idx: self.function_idx,
            function_name_wasm: self.function_name_wasm,
            function_name_dwarf: self.function_name_dwarf,
            param_or_return: self.param_or_return,
            wasm_type: self.wasm_type,
            wasm_body: f(self.wasm_body),
            type_: self.type_,
            aux: self.aux,
        }
    }
    pub fn with_wasm_body<R>(self, wasm_body: R) -> WasmTypeSample<R, U, V> {
        self.map_wasm_body(|_| wasm_body)
    }

    pub fn map_type<R>(self, f: impl FnOnce(U) -> R) -> WasmTypeSample<T, R, V> {
        WasmTypeSample {
            file: self.file,
            compilation_unit: self.compilation_unit,
            function_idx: self.function_idx,
            function_name_wasm: self.function_name_wasm,
            function_name_dwarf: self.function_name_dwarf,
            param_or_return: self.param_or_return,
            wasm_type: self.wasm_type,
            wasm_body: self.wasm_body,
            type_: f(self.type_),
            aux: self.aux,
        }
    }
    pub fn with_type<R>(self, type_: R) -> WasmTypeSample<T, R, V> {
        self.map_type(|_| type_)
    }
}

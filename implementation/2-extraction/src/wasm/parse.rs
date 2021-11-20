use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::anyhow;
use wasmparser::{FunctionBody, ImportSectionEntryType, NameSectionReader, Operator, Parser, TypeDef};

/// Extracted information about a WebAssembly binary. Borrows from some underlying data.
#[derive(Debug, Clone)]
pub struct WasmBinary<'a> {
    pub code_section_offset: usize,
    pub custom_sections: HashMap<&'a str, Rc<[u8]>>,
    pub functions: Vec<WasmFunction>,
    // Make the function names a shared pointer already here, because they will be shared across
    // all parameters and the return type samples.
    pub function_names: HashMap<u32, Arc<str>>,
}

#[derive(Debug, Clone)]
pub struct WasmFunction {
    pub idx: u32,
    pub type_: wasmparser::FuncType,
    // Unfortunately, wasmparser::FunctionBody<'a> and even individual wasmparser::Operator<'a>
    // borrow from the underlying bytes, which adds a lifetime to this struct, which would make
    // it impossible to return it from a function where the bytes are a local variable.
    // (Or alternatively, storing the bytes also here would make it self-borrowing, also not OK :-( )
    // Thus, we store the raw bytes of the body in this WasmBody struct instead of borrowing it.
    pub body: WasmBody,
}

/// Not-yet parsed representation of WebAssembly function bodies.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct WasmBody {
    // We use Rc instead of Box/Vec for the bytes, such that we can share the same function body 
    // across multiple function parameters/return type samples later, without copying.
    pub offset: usize,
    pub bytes: Rc<[u8]>,

    // TODO Add lookup tables global, local, function types.
}

// Do not write the raw body bytes to debug output, since (1) I cannot read WebAssembly byte code
// directly (yet :P) and (2) that easily overwhelms the console.
// One easy way to look at the WebAssembly instructions is wasm2wat/wasm-objdump + function idx.
impl fmt::Debug for WasmBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmBody")
            .field("offset", &format!("0x{:x}", self.offset))
            .field("bytes", &"...")
            .finish()
    }
}

impl WasmBody {
    pub fn from(body: FunctionBody, bytes: &[u8]) -> Self {
        let offset = body.range().start;
        let bytes = Rc::from(body.range().slice(bytes));
        Self { offset, bytes }
    }

    pub fn instructions(&self) -> wasmparser::Result<impl Iterator<Item = wasmparser::Result<Operator>>> {
        let body = FunctionBody::new(self.offset, &self.bytes);
        let iter = body.get_operators_reader()?
            .into_iter_with_offsets()
            .map(|result| 
                result.map(|(op, _offset)| op));
        Ok(iter)
    }
}

impl<'a> WasmBinary<'a> {
    pub fn parse(bytes: &'a [u8]) -> anyhow::Result<Self> {
        // Result fields.
        let mut custom_sections = HashMap::new();
        let mut functions = Vec::new();
        let mut function_names = HashMap::new();

        // Global state during parsing.
        let mut code_section_offset = None;

        let mut imported_function_count = 0;
        let mut local_function_count = 0;

        // Since those two maps will be dense, use a Vec as representation.
        let mut local_function_idx_to_type_idx = Vec::new();
        let mut type_idx_to_type = Vec::new();

        let parser = Parser::new(0);
        for payload in parser.parse_all(&bytes) {
            use wasmparser::Payload::*;
            match payload? {
                // Keep a map of ty index -> type for resolving function types.
                TypeSection(mut reader) => {
                    for _ in 0..reader.get_count() {
                        let ty = reader.read()?;
                        let func_ty = match ty {
                            TypeDef::Func(func_ty) => Some(func_ty),
                            _ => None,
                        };
                        type_idx_to_type.push(func_ty);
                    }
                }
                // Keep number of imported functions for offsetting function indices.
                ImportSection(mut reader) => {
                    for _ in 0..reader.get_count() {
                        let import = reader.read()?;
                        if let ImportSectionEntryType::Function(..) = import.ty {
                            imported_function_count += 1;
                        }
                    }
                }
                // Keep a map of func idx (without the imported functions!) -> ty idx for resolving function types.
                FunctionSection(mut reader) => {
                    for _local_function_idx in 0..reader.get_count() {
                        let ty_idx = reader.read()?;
                        local_function_idx_to_type_idx.push(ty_idx);
                    }
                }
                // Keep the start of the code section for computing relative function offsets (for DWARF).
                CodeSectionStart { range, .. } => {
                    match code_section_offset {
                        None => code_section_offset = Some(range.start),
                        Some(prev_offset) => anyhow::bail!("more than one code section, previous one was at offset {}", prev_offset)
                    };
                }
                CodeSectionEntry(body) => {
                    // Store all local functions (i.e., functions with a body) with their idx, type, offset etc.
                    // This assumes all types, imports, and function types were already processed, which
                    // is true for valid binaries because of WebAssembly's section order.
                    let function_idx = imported_function_count + local_function_count;

                    let type_idx = *local_function_idx_to_type_idx.get(local_function_count as usize)
                        .ok_or_else(|| anyhow!("missing type index for function index {}", function_idx))?;
                    let type_ = type_idx_to_type.get(type_idx as usize)
                        .ok_or_else(|| anyhow!("missing type for type index {}", type_idx))?
                        .as_ref()
                        .ok_or_else(|| anyhow!("not a function type at type index {}", type_idx))?
                        .clone();
                    
                    functions.push(WasmFunction {
                        idx: function_idx,
                        type_,
                        body: WasmBody::from(body, bytes),
                    });

                    local_function_count += 1;
                }
                // Keep a map of function idx -> name.
                CustomSection { name, data, data_offset } => {
                    custom_sections.insert(name, Rc::from(data));
                    if name == "name" {
                        parse_name_section(data, data_offset, &mut function_names)?;
                    }
                }
                _ => {}
            }
        }

        let code_section_offset = code_section_offset.ok_or_else(|| anyhow!("missing code section"))?;

        Ok(WasmBinary { code_section_offset, custom_sections, functions, function_names })
    }
}

fn parse_name_section(
    data: &[u8], 
    section_offset: usize, 
    function_names: &mut HashMap<u32, Arc<str>>
) -> anyhow::Result<()> {
    let mut reader = NameSectionReader::new(data, section_offset)?;
    while !reader.eof() {
        use wasmparser::Name::*;
        match reader.read() {
            Ok(Module(_)) => {}
            Ok(Function(function_names_subsection)) => {
                let mut reader = function_names_subsection.get_map()?;
                for _ in 0..reader.get_count() {
                    let naming = reader.read()?;
                    let duplicate_name = function_names.insert(naming.index, Arc::from(naming.name));
                    if let Some(duplicate_name) = duplicate_name {
                        anyhow::bail!("duplicate name for function {}: '{}' and '{}'", naming.index, duplicate_name, naming.name);
                    }
                }
            }
            Ok(Local(_)) => {
                // TODO Also extract parameter names from name section?
                // For now, parameter names come from the DWARF info only, which makes sense for
                // debugging information, which also comes from DWARF.
            }
            // Ignore errors when reading the name section, bacuse those could be from the (still 
            // non-standard) "extended name section".
            // https://github.com/WebAssembly/extended-name-section/blob/master/proposals/extended-name-section/Overview.md
            _ => {
                // But stop parsing after the first error, otherwise it might  
                // hypnotize weird function names.
                break;
            }, 
        }
    }
    Ok(())
}

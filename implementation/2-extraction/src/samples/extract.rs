use std::path::Path;
use std::sync::Arc;

use gimli::{DW_AT_name, EndianRcSlice, LittleEndian};
use itertools::Itertools;

use crate::dwarf::parse::DwarfBinary;
use crate::dwarf::util::DwarfEntry;
use crate::wasm::parse::{WasmBinary, WasmBody};
use crate::samples::sample::{WasmTypeSample, ParamOrReturn};

pub fn extract_samples(file: &Path) -> anyhow::Result<
    impl Iterator<Item = 
        gimli::Result<
            WasmTypeSample<
                WasmBody, 
                DwarfEntry<EndianRcSlice<LittleEndian>>
            >
        >
    >
> {
    let bytes = std::fs::read(file)?;

    let file: Arc<Path> = Arc::from(file);

    let wasm = WasmBinary::parse(&bytes)?;
    let code_section_offset = wasm.code_section_offset;
    let mut wasm_function_names = wasm.function_names;

    let mut dwarf = DwarfBinary::parse(&wasm.custom_sections)?;

    let iter = 
        wasm.functions
        .into_iter()

        // Match up WebAssembly functions with DWARF functions via their offsets.
        .filter_map(move |wasm| {
            let relative_offset = wasm.body.offset - code_section_offset;
            let has_dwarf = dwarf.relative_offsets_to_function_entries.remove(&relative_offset);
            has_dwarf.map(|dwarf| (wasm, dwarf))
        })
        
        // Remove functions where the Wasm and DWARF types do not align.
        .filter(|(wasm, dwarf)| {
            let params_same_len = wasm.type_.params.len() == dwarf.params.len();

            // Originally, I wanted to remove also all samples where the return type does not align
            // (i.e., not both void xor both non-void). However, in quite many samples where the
            // parameter lists DO align, the return types DO NOT, because the WebAssembly function
            // returns something that is neither declared in the source or debug info (example: 
            // maxtrax.o, function #13 _ZN5Audio7MaxTraxC2Eibtt). I think this is some compiler
            // optimization adding the this pointer as the return value of methods.
            // Since we do not want to loose that many samples, we do NOT check/align returns here,
            // (i.e., those functions are still included for their parameters), but DO check later
            // when extracting return types (which we only do if both Wasm and DWARF have one set).
            let return_same_len = 
                // For now, we only support the WebAssembly MVP with a single return value.
                (wasm.type_.returns.len() == 1 && dwarf.return_type.is_some())
                || (wasm.type_.returns.len() == 0 && dwarf.return_type.is_none());
            let return_same_len = true || return_same_len;

            params_same_len && return_same_len
        })

        .flat_map(move |(wasm_function, dwarf_function)| {
            // I am not sure why I need to clone this here instead of just in the closure below?
            let file = Arc::clone(&file);

            // Destructure wasm_function and dwarf_function to make borrowck happy for the closure below.
            let function_idx = wasm_function.idx;

            let function_name_wasm = wasm_function_names.remove(&function_idx);
            let function_name_dwarf = dwarf_function.name;

            let compilation_unit = dwarf_function.compilation_unit_name;

            let wasm_body = wasm_function.body;

            let dwarf_params = dwarf_function.params;
            // TODO Unfortunately, Box<[T]>::into_iter() does NOT move out of self (it only borrows 
            // -,-), so go through a Vec instead, where into_iter() is correct and satisfies the 
            // borrow checker (we cannot borrow wasm_function in the returned iterator).
            let wasm_params = Vec::from(wasm_function.type_.params);

            let params_iter = 
                wasm_params.into_iter()
                .zip_eq(dwarf_params.into_iter())
                .enumerate()
                .map(move |(idx, (wasm, dwarf))| -> gimli::Result<_> {
                    let name = dwarf.attr_str(DW_AT_name)?;
                    Ok((wasm, dwarf, ParamOrReturn::Param { idx: idx as u32, name }))
                });

            // Extract a return type sample only if both WebAssembly and DWARF have a return type.
            let wasm_return = wasm_function.type_.returns.get(0).cloned();
            let dwarf_return = dwarf_function.return_type;
            let return_ = 
                wasm_return
                .zip(dwarf_return)
                .map(|(wasm, dwarf)| 
                    Ok((wasm, dwarf, ParamOrReturn::Return)));

            let samples_iter = 
                params_iter
                .chain(return_);

            samples_iter.map_ok(move |(wasm, dwarf, param_or_return)| 
                WasmTypeSample {
                    file: Arc::clone(&file),
                    compilation_unit: compilation_unit.clone(),
                    function_idx,
                    function_name_wasm: function_name_wasm.clone(),
                    function_name_dwarf: function_name_dwarf.clone(),
                    wasm_type: wasm,
                    type_: dwarf,
                    wasm_body: wasm_body.clone(),
                    param_or_return,
                    aux: ()
                }
            )
        });

    Ok(iter)
}

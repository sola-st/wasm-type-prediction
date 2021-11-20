use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;

use gimli::{AttributeValue, EndianRcSlice, LittleEndian, Reader, SectionId};
use gimli::constants::*;
use itertools::Itertools;

use crate::dwarf::util::DwarfEntry;

#[derive(Debug)]
pub struct DwarfBinary<R: Reader> {
    pub relative_offsets_to_function_entries: HashMap<usize, DwarfFunction<R>>
}

impl DwarfBinary<EndianRcSlice<LittleEndian>> {
    pub fn parse(sections: &HashMap<&str, Rc<[u8]>>) -> anyhow::Result<Self> {
        let dwarf = {
            // Identify DWARF sections by their custom section name.
            let loader = |dwarf_section: SectionId| -> Result<_, Infallible> {
                let data = sections.get(dwarf_section.name())
                    .cloned()
                    .unwrap_or(Rc::from([]));

                Ok(EndianRcSlice::new(data, LittleEndian))
            };

            // We don't have a supplementary object file.
            let sup_loader = |_| Ok(EndianRcSlice::new(Rc::from([]), LittleEndian));

            gimli::Dwarf::load(loader, sup_loader)
        }?;
        let dwarf = Rc::new(dwarf);

        let mut relative_offsets_to_function_entries: HashMap<usize, DwarfFunction<EndianRcSlice<LittleEndian>>> =  HashMap::new();
        let mut relative_offsets_with_inconsistent_entries = HashSet::new();

        // Iterate over all compilation units.
        let mut units_iter = dwarf.units();
        while let Some(unit_header) = units_iter.next()? {
            let unit = dwarf.unit(unit_header)?;
            let unit_name = match &unit.name {
                Some(name) => Some(Arc::from(name.to_string()?)),
                None => None,
            };
            let unit = Rc::new(unit);

            // Iterate over all DWARF tags in depth-first order.
            let mut entries = unit.entries();
            while let Some((_delta_depth, entry)) = entries.next_dfs()? {

                // Look for all functions in this compilation unit.
                if entry.tag() == DW_TAG_subprogram {
                
                    // Save only those with a location (i.e., that can be potentially mapped to WebAssembly).
                    let location = entry.attr_value(DW_AT_low_pc)?;
                    match location {
                        None => {}
                        Some(AttributeValue::Addr(relative_offset)) => {
                            let relative_offset = relative_offset as usize;
                            let entry = DwarfEntry::from(&dwarf, &unit, entry);
                            let function = DwarfFunction::from(unit_name.clone(), &entry)?;

                            // In some binaries (e.g., scummvm.wasm) there are multiple DWARF 
                            // entries for the same WebAssembly function (by relative_offset).
                            // Originally, we just chose one arbitrary DWARF entry to keep (e.g.,
                            // the last encountered one), but now, we try to make sure the
                            // duplicate DWARF entries are at least consistent with each other.
                            // Another option would have been to remove all samples where there is
                            // more than one DWARF entry, but that removes quite a lot (20% samples), 
                            // where we suspect the compiler just duplicated/copied DWARF info
                            // for whatever reason (example: scummvm.wasm, multiple entries with 
                            // DW_AT_low_pc = 0x01002d1f, but all containing essentially the same info).
                            if let Some(previous) = relative_offsets_to_function_entries.get(&relative_offset) {
                                // Ensure that the previous one and this entry are consistent (contain
                                // the same debug information), at least on a superficial level.
                                let same_name = previous.name == function.name;
                                let same_param_count = previous.params.len() == function.params.len();
                                let same_return_count = previous.return_type.iter().count() == function.return_type.iter().count();
                                if !(same_name && same_param_count && same_return_count) {
                                    relative_offsets_with_inconsistent_entries.insert(relative_offset);
                                }
                            } else {
                                relative_offsets_to_function_entries.insert(relative_offset, function);
                            }
                        }

                        Some(attr_value) => unimplemented!("unknown attribute value for DW_AT_low_pc: {:?}", attr_value)
                    }
                }
            }
        }

        for relative_offset in relative_offsets_with_inconsistent_entries {
            relative_offsets_to_function_entries.remove(&relative_offset);
            
            // Quite some DWARF entries have 0x0 as the location set for many different functions.
            // Those are maybe because of relocatable .wasm files (not sure), so ignore them as an error.
            // However, if there are non-0x0, duplicate, inconsistent DWARF entries, fail.
            if relative_offset != 0 {
                anyhow::bail!("duplicate, inconsistent DWARF entries for function at relative offset 0x{:x}", relative_offset);
            }
        }

        Ok(DwarfBinary { relative_offsets_to_function_entries })
    }
}

#[derive(Debug, Clone)]
pub struct DwarfFunction<R: Reader> {
    pub compilation_unit_name: Option<Arc<str>>,
    // Make it directly a shared pointer (instead of String or Box<str>), because the function name
    // will be shared across all parameter/return type samples from this function and that saves
    // one copy.
    pub name: Option<Arc<str>>,

    pub params: Vec<DwarfEntry<R>>,
    pub return_type: Option<DwarfEntry<R>>,
}

impl<R: Reader> DwarfFunction<R> {
    pub fn from(
        compilation_unit_name: Option<Arc<str>>,
        function_entry: &DwarfEntry<R>
    ) -> anyhow::Result<Self> {
        // If the function has an DW_AT_abstract_origin attribute (e.g., it is the inlined version
        // or monomorphized of some generic one), use that instead, because they often have more
        // information on their nodes (e.g., names and types of parameters, return types).
        // For parameters, I have similar handling to this in `Type::parse()`, but for return types
        // this didn't work, because the DW_AT_type attribute is just missing on the non-abstract function.
        if let Some(abstract_origin) = function_entry.attr_entry(DW_AT_abstract_origin)? {
            return Self::from(compilation_unit_name, &abstract_origin);
        }

        let name = function_entry.attr_str(DW_AT_name)?.map(Arc::from);

        let params = function_entry.children()?
            .filter_ok(|entry| entry.tag == DW_TAG_formal_parameter)
            .try_collect()?;
        let return_type = function_entry.attr_entry(DW_AT_type)?;
        
        Ok(DwarfFunction { compilation_unit_name, name, params, return_type })
    }
}

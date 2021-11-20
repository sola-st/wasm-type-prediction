use std::fmt;

use itertools::Itertools;
use sha2::{Digest, Sha256};
use wasmparser::Operator;
use rand::prelude::{SliceRandom, StdRng};

use crate::samples::sample::{WasmTypeSample, ParamOrReturn};
use crate::wasm::fmt::{type_str, fmt_instr};
use crate::wasm::parse::WasmBody;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WasmRepr {
    Hash(Option<wasmparser::Type>, Box<str>),
    Full(Option<wasmparser::Type>, Box<str>),
    Subrange(Option<wasmparser::Type>, Box<str>),
    Windows(Option<wasmparser::Type>, Vec<Box<str>>)
}

impl WasmRepr {
    pub fn new_hash<T, U>(sample: &WasmTypeSample<WasmBody, T, U>, with_type: bool) -> Self {
        let hash = format!("{:x}", Sha256::digest(&sample.wasm_body.bytes[..])).into();
        
        let with_type = with_type.then(|| sample.wasm_type);
        Self::Hash(with_type, hash)
    }

    pub fn new_full<T, U>(sample: &WasmTypeSample<WasmBody, T, U>, with_type: bool) -> anyhow::Result<Self> {
        let instructions: Vec<_> = sample.wasm_body.instructions()?.try_collect()?;
        
        let str = Self::instructions_to_string(&instructions, &sample.param_or_return)?;

        let with_type = with_type.then(|| sample.wasm_type);
        Ok(Self::Full(with_type, str))
    }

    pub fn new_subrange<T, U>(sample: &WasmTypeSample<WasmBody, T, U>, n_instructions: usize, with_type: bool) -> anyhow::Result<Self> {
        // Take the first n instructions for parameters, and the last n for returns.
        let instructions = match sample.param_or_return {
            ParamOrReturn::Param { .. } => {
                sample.wasm_body.instructions()?.take(n_instructions).try_collect()?
            },
            ParamOrReturn::Return => {
                // Put the instructions into a vector first, such that we can get the last N.
                let instructions: Vec<_> = sample.wasm_body.instructions()?.try_collect()?;
                instructions.into_iter().rev().take(n_instructions).rev().collect_vec()
            }
        };

        let str = Self::instructions_to_string(&instructions, &sample.param_or_return)?;

        let with_type = with_type.then(|| sample.wasm_type);
        Ok(Self::Subrange(with_type, str))
    }

    pub fn new_windows<T, U>(sample: &WasmTypeSample<WasmBody, T, U>, window_size: usize, with_type: bool, rng: &mut StdRng) -> anyhow::Result<Self> {
        // Left and right pad instructions, for functions that are shorter than the window size.
        let mut padded_instructions = vec![None; window_size];
        let mut instruction_count = 0;
        for op in sample.wasm_body.instructions()? {
            padded_instructions.push(Some(op?));
            instruction_count += 1;
        }
        padded_instructions.extend(std::iter::repeat(None).take(window_size));

        let mut windows = Vec::new();
        for (i, window) in padded_instructions.windows(window_size).enumerate() {
            use wasmparser::Operator::*;
            let extract = match &sample.param_or_return {
                ParamOrReturn::Param { idx, .. } => {
                    // Window _around_ a parameter local access for parameter samples.
                    match &window[window_size/2] {
                        Some(LocalGet { local_index })
                        | Some(LocalSet { local_index })
                        | Some(LocalTee { local_index }) => local_index == idx,
                        _ => false
                    }
                }
                ParamOrReturn::Return => {
                    // Window _before_ (ending with) return instructions.
                    let return_window = match window.last() {
                        Some(Some(Return)) => true,
                        _ => false,
                    };

                    // If there is no explicit return, just use the last window (before padding).
                    // -1 because we don't want the end instruction in the window (which is always at the end of the function) and also don't want to replicate the window twice, if there WAS a return
                    let is_last_window_before_end = i == instruction_count - 1; 

                    return_window || is_last_window_before_end
                }
            };

            // Filter out padding, because that only uses up "token space":
            let window = window.into_iter().filter_map(|option| option.as_ref());
            
            if extract {
                windows.push(Self::instructions_to_string(window, &sample.param_or_return)?);
            }
        }

        // TODO Filter out windows that are overlapping by more than x% of the window size.
        // Idea/steps: 1. save window start index with each window, 2. iterate over subsequent pairs
        // of windows, 3. take the first element of the pair only, if delta(index, index_next) < threshold.

        // Reorder windows such that closeby windows do not end up next to each other.
        // TODO Alternative to random shuffling: reverse Z-order curve, which maps far away items
        // close to each other, with decreasing "frequency".
        windows.shuffle(rng);

        let with_type = with_type.then(|| sample.wasm_type);
        Ok(Self::Windows(with_type, windows))
    }

    fn instructions_to_string<'a, 'b : 'a>(instructions: impl IntoIterator<Item=&'a Operator<'b>>, abstract_param: &ParamOrReturn) -> anyhow::Result<Box<str>> {
        let instructions = instructions.into_iter();

        // Pre-allocate string: one instruction is about 6 (?) characters.
        let mut str = String::with_capacity(instructions.size_hint().1.unwrap_or(0) * 6);

        // Abstract parameter index in local.* instructions to <param>, no abstraction for returns.
        let param_local_idx = match abstract_param {
            ParamOrReturn::Param { idx, .. } => Some(*idx),
            ParamOrReturn::Return => None
        };

        for op in instructions {
            fmt_instr(&mut str, &op, param_local_idx)?;
            str.push_str(" ; ");
        }
        // Remove last trailing seperator.
        str.pop(); str.pop(); str.pop();

        // Compact the string to a Box<str>, because we will never append anything to it from here on.
        Ok(str.into())
    }

    fn type_(&self) -> Option<wasmparser::Type> {
        match self {
            WasmRepr::Hash(ty, _) 
            | WasmRepr::Full(ty, _)
            | WasmRepr::Subrange(ty, _)
            | WasmRepr::Windows(ty, _) => ty.clone()
        }
    }
}

impl fmt::Display for WasmRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ty) = self.type_() {
            write!(f, "{} <begin> ", type_str(ty))?;
        }
        match self {
            WasmRepr::Hash(_, hash) => f.write_str(hash),
            WasmRepr::Full(_, str) => f.write_str(str),
            WasmRepr::Subrange(_, str) => f.write_str(str),
            WasmRepr::Windows(_, windows) => {
                if let Some((last_window, windows)) = windows.split_last() {
                    for window in windows {
                        f.write_str(window)?;
                        f.write_str(" <window> ")?;
                    }
                    f.write_str(last_window)?;
                }
                Ok(())
            }
        }
    }
}
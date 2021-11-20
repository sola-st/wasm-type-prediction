use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};
use wasmparser::{Parser, Payload};

use crate::util::file_error::{FileError, ResultWithFile};

#[derive(Debug, Clone)]
pub struct WasmBinaryStats {
    pub file_size: u64,
    pub file_sha256: Box<[u8]>,

    pub instruction_count: u64,

    pub function_bodies_count: u64,
    pub function_bodies_bytes: u64,

    pub binary_signature: Box<[u8]>,
}

impl WasmBinaryStats {
    // Wrap inner function to attach filename to error.
    pub fn from_file(file: impl AsRef<Path>) -> Result<Self, FileError<anyhow::Error>> {
        Self::from_file_inner(file.as_ref()).with_file(file)
    }

    fn from_file_inner(file: impl AsRef<Path>) -> anyhow::Result<Self> {
        let bytes = std::fs::read(file)?;
        
        let file_sha256 = Sha256::digest(&bytes).as_slice().into();

        let mut instruction_count = 0;

        let mut function_bodies_count = 0;
        let mut function_bodies_bytes = 0;
        let mut function_bodies_hashes = Vec::new();

        let wasm_parser = Parser::new(0);
        for payload in wasm_parser.parse_all(&bytes) {
            let payload = payload?;
            if let Payload::CodeSectionEntry(function_body) = payload {
                function_bodies_count += 1;

                let function_body_bytes = function_body.range().slice(&bytes);
                function_bodies_bytes += function_body_bytes.len() as u64;

                // Compute hash over names of instructions, this abstracts away differences in function indices etc.
                let mut hasher = Sha256::new();
                let mut reader = function_body.get_operators_reader()?;
                while !reader.eof() {
                    let op = reader.read()?;
                    let instruction_name_only = crate::wasm::fmt::instr_name(&op);
                    hasher.write(instruction_name_only.as_bytes())?;
                }
                let function_instruction_names_sha256 = hasher.finalize();

                // TODO Alternative that is more strict, i.e., produces less duplicates:
                // Take the full bytes of the body for producing the hash. This does not abstract
                // over (function/local/global) indices or instruction immediates (constants).
                let _function_body_sha256 = Sha256::digest(function_body_bytes);
                
                function_bodies_hashes.extend(function_instruction_names_sha256);

                let mut reader = function_body.get_operators_reader()?;
                while !reader.eof() {
                    let _op = reader.read()?;
                    instruction_count += 1;
                }
            }
        }

        let binary_signature = Sha256::digest(&function_bodies_hashes).as_slice().into();

        Ok(WasmBinaryStats {
            file_size: bytes.len() as u64,
            file_sha256,
            instruction_count,
            function_bodies_count,
            function_bodies_bytes,
            binary_signature
        })
    }
}

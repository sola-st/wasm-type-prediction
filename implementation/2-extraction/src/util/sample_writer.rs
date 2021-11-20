use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::{fs, io};

use itertools::Itertools;
use rand::prelude::{SliceRandom, StdRng};
use rayon::slice::ParallelSliceMut;

use serde::Serialize;

use crate::samples::sample::{ParamOrReturn, WasmTypeSample};
use crate::samples::types::Type;
use crate::samples::wasm_repr::WasmRepr;

// Struct for quick implementation of serialization to JSON with serde.
#[derive(Debug, Serialize)]
struct SampleInfo<'a> {
    file: &'a str,
    compilation_unit: Option<&'a str>,
    function_idx: u32,
    function_name_wasm: Option<&'a str>,
    function_name_dwarf: Option<&'a str>,
    // None/null (JSON) if this is a return type sample.
    param_idx: Option<u32>,
    param_name: Option<&'a str>,
}

impl<'a> SampleInfo<'a> {
    pub fn from<T, U, R>(sample: &'a WasmTypeSample<T, U, R>) -> Self {
        let (param_idx, param_name) = match &sample.param_or_return {
            ParamOrReturn::Param { idx, name } => (Some(*idx), name.as_deref()),
            ParamOrReturn::Return => (None, None)
        };
        Self {
            file: sample.file.to_str().unwrap(),
            compilation_unit: sample.compilation_unit.as_deref(),
            function_idx: sample.function_idx,
            function_name_wasm: sample.function_name_wasm.as_deref(),
            function_name_dwarf: sample.function_name_dwarf.as_deref(),
            param_idx,
            param_name
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum WasmTypeInfo {
    Wasm,
    Type,
    Info,
}
impl WasmTypeInfo {
    pub fn to_str(self) -> &'static str {
        match self {
            WasmTypeInfo::Wasm => "wasm",
            WasmTypeInfo::Type => "type",
            WasmTypeInfo::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ParamReturn {
    Param,
    Return,
}
impl ParamReturn {
    pub fn to_str(self) -> &'static str {
        match self {
            ParamReturn::Param => "param",
            ParamReturn::Return => "return",
        }
    }
}

/// Convenience wrapper around output files: 3 wasm/dwarf/info * 2 param/return.
pub struct SampleWriter {
    writers: HashMap<(WasmTypeInfo, ParamReturn), BufWriter<File>>,
}

impl SampleWriter {
    pub fn create_files(directory: impl AsRef<Path>) -> io::Result<Self> {
        let mut writers = HashMap::new();

        use WasmTypeInfo::*;
        use ParamReturn::*;
        for &wti in &[Wasm, Type, Info] {
            for &pr in &[Param, Return] {
                let writer = Self::create_file(&directory, wti, pr)?;
                writers.insert((wti, pr), writer);
            }
        }

        Ok(SampleWriter { writers })
    }

    /// Create a file like output_dir/param/wasm.txt
    fn create_file(output_dir: impl AsRef<Path>, wti: WasmTypeInfo, pr: ParamReturn) -> io::Result<BufWriter<File>> {
        // Make sure the parent directories exist.
        let dir = output_dir.as_ref().join(pr.to_str());
        fs::create_dir_all(&dir)?;

        use WasmTypeInfo::*;
        let extension = match wti {
            Wasm | Type => "txt",
            Info => "jsonl"
        };
        let filename = format!("{}.{}", wti.to_str(), extension);
        let path = dir.join(filename);

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Ok(writer)
    }

    pub fn write(&mut self, sample: &WasmTypeSample<WasmRepr, Type, ()>) -> io::Result<()> {
        use ParamReturn::*;
        let pr = match sample.param_or_return {
            ParamOrReturn::Param { .. } => Param,
            ParamOrReturn::Return => Return
        };

        // Write WebAssembly input, type output, and sample info for debugging.
        use WasmTypeInfo::*;
        writeln!(self.writers.get_mut(&(Wasm, pr)).unwrap(), "{}", sample.wasm_body)?;
        writeln!(self.writers.get_mut(&(Type, pr)).unwrap(), "{}", sample.type_)?;
        
        let info = SampleInfo::from(sample);
        let mut info_writer = self.writers.get_mut(&(Info, pr)).unwrap();
        serde_json::to_writer(&mut info_writer, &info)?;
        writeln!(info_writer)?;

        Ok(())
    }

    /// Flushes all underlying writers and reports the number of bytes written to all files combined.
    pub fn bytes_written(&mut self) -> io::Result<u64> {
        let mut bytes_written = 0;
        for writer in self.writers.values_mut() {
            writer.flush()?;
            // BufWriter and File implement Seek, from which we can get the current position:
            // https://stackoverflow.com/questions/42187591/how-to-keep-track-of-how-many-bytes-written-when-using-stdiowrite
            bytes_written += writer.seek(SeekFrom::Current(0))?;
        }
        Ok(bytes_written)
    }
}

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Clap;
use itertools::Itertools;
use rand::prelude::StdRng;
use rand::SeedableRng;
use walkdir::WalkDir;
use anyhow::bail;

use crate::util::cmultimap::CMultiMap;
use crate::util::sample_writer::SampleWriter;
use crate::util::percent::Percent;

#[derive(Clap, Debug)]
#[clap(
    author = clap::crate_authors!(),
    version = clap::crate_version!(),
    about = clap::crate_description!(),
    setting = clap::AppSettings::DeriveDisplayOrder
)]
pub struct Options {

    // General options:

    /// Input files and directories. Directories are recursively searched for WebAssembly binaries.
    #[clap(required = true)]
    inputs: Vec<PathBuf>,

    /// Directory for all output files (training data, logs, baseline model etc.).
    #[clap(long, short, default_value = "out/", value_name = "path")]
    output_dir: PathBuf,

    /// Write additional logfile to the output directory [default: false].
    /// Optionally, also set the filename of the log via the argument [default: current datetime].
    #[clap(long, short, value_name = "filename")]
    log: Option<Option<String>>,

    /// Print also debug output.
    #[clap(long, short)]
    pub verbose: bool,

    /// Number of entries to print for distributions like common types, names, etc.
    #[clap(long, default_value = "20", value_name = "N")]
    pub stats_max: usize,

    /// Seed for RNG to make random operations reproducible (e.g., shuffling, subsampling data).
    #[clap(long, default_value = "0", value_name = "N")]
    rand_seed: u64,


    // Options for WebAssembly input representation:

    /// Filter out samples from the whole dataset where the parameter is never used in the 
    /// WebAssembly function.
    /// FIXME do not use parse(try_from_str) for bool, since it DOES NOT WARN if you give the option without argument and sets it to false!
    #[clap(long, parse(try_from_str), default_value = "true", value_name = "true|false")]
    pub wasm_filter_unused_param: bool,

    /// Add raw WebAssembly type of the parameter to predict to the input data.
    #[clap(long, parse(try_from_str), default_value = "true", value_name = "true|false")]
    pub wasm_add_raw_type: bool,

    /// Representation of WebAssembly function bodies.
    /// "hash": hash of the body's bytes, useful for statistics on raw bodies, e.g., task-inherent non-determinism.
    /// "full": all instructions in the body.
    /// "subrange": first N instructions (for parameter types) and last N instructions (for return values).
    /// "windows": windows of size N around each parameter usage or return instruction (respectively).
    #[clap(long, arg_enum, value_name = "repr")]
    wasm_repr: WasmReprOption,

    /// For the WebAssembly representations 'subrange' and 'windows', the size parameter, i.e.,
    /// the length of the subrange and the size of each window, respectively.
    #[clap(long, value_name = "N")]
    wasm_repr_size: Option<usize>,
        
    // /// Add raw WebAssembly types of calls, locals, and globals to the input data.
    // #[clap(long, parse(try_from_str), default_value = "false", value_name = "true|false")]
    // pub wasm_add_raw_types_other: bool,


    // Options for the output types and type language:

    /// Filter out samples from the whole dataset where the type is unknown.
    #[clap(long, parse(try_from_str), default_value = "true", value_name = "true|false")]
    pub type_filter_unknown: bool,

    /// Remove typedef and nominal nodes from the types.
    #[clap(long, parse(try_from_str), default_value = "false", value_name = "true|false")]
    pub type_remove_names: bool,

    /// Remove const from types, i.e., equate const and non-const types.
    /// Removing const retains less information about the source program, but makes prediction easier.
    #[clap(long, parse(try_from_str), default_value = "false", value_name = "true|false")]
    pub type_remove_const: bool,

    /// Map/equate class types to struct types, i.e., do NOT keep them as separate types.
    /// Mapping classes to structs retains less information about the source program, but makes 
    /// prediction easier.
    #[clap(long, parse(try_from_str), default_value = "false", value_name = "true|false")]
    pub type_class_to_struct: bool,

    /// How to handle typedefs: Keeping them as-is, converting them to nominal types, or removing
    /// them altogether (essentially equating all typedefs of the same inner type).
    #[clap(long, arg_enum, default_value = "keep", value_name = "keep|to-nominal|remove")]
    pub type_typedefs: Typedefs,

    /// Save statistics about all typedef and nominal type names as a CSV [default: false].
    /// Optionally, also set the filename via the argument [default: 'name-stats.csv'].
    #[clap(long, value_name = "filename")]
    type_save_name_stats: Option<Option<String>>,

    /// Remove all typedef and nominal names that are not on the given name list file (1 name per line, no markup).
    #[clap(long, value_name = "path")]
    type_keep_name_list: Option<String>,

    /// Keep at most one typedef or nominal name if there are multiple, namely the outermost one.
    #[clap(long, parse(try_from_str), default_value = "false", value_name = "true|false")]
    pub type_name_flatten_outermost: bool,

    // TODO use the following options

    // /// Remove the representation of a nominal type by truncating after the 'name' token.
    // type_nominal_remove_repr: bool,

    // Options for configuring the training data output, e.g., how much auxiliary information is
    // added, thresholds for cutting off stuff, which "training task" etc.

    // TODO data-driven type simplification: build prefix tree of types, merge those which account for less than 1% of data
    // pub type_datadriven_simplification: bool,
    
    // /// Maximum number of linear OR tree-like type constructors before cut off.
    // // pub type_max_depth: Option<u32>,
    // /// Maximum number of tree-like type constructors before cut off, default: 0
    // // pub type_max_tree_depth: Option<u32>,

    // /// After having split dataset into train/dev/test set, remove samples from train that appear verbatim in dev/test set
    // // pub deduplicate_dev_test_samples: bool
}

#[derive(Clap, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Typedefs {
    Keep,
    ToNominal,
    Remove,
}

#[derive(Clap, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WasmReprOption {
    Hash,
    Full,
    Subrange,
    Windows,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WasmRepr {
    Hash,
    Full,
    Subrange(usize),
    Windows(usize)
}

impl Options {
    /// Iterator over all input files in the given files and directories.
    pub fn input_files(&self) -> impl Iterator<Item = Result<PathBuf, walkdir::Error>> + '_ {
        self.inputs
            .iter()
            // Look recursively for files in every given input path.
            .flat_map(|input| WalkDir::new(input))
            // Keep only files, not directories.
            .filter_ok(|entry| entry.file_type().is_file())
            .map_ok(|entry| entry.into_path())
    }

    /// Get a (deterministic) RNG with the seed given in the options.
    pub fn rng_with_seed(&self) -> StdRng {
        StdRng::seed_from_u64(self.rand_seed)
    }

    /// Create (buffered) output files, overwriting existing ones in the output directory.
    pub fn sample_writer(&self) -> io::Result<SampleWriter> {
        SampleWriter::create_files(&self.output_dir)
    }

    /// Create a logfile in the output directory, if logging to file was requested.
    pub fn create_log_file(&self) -> Option<io::Result<File>> {
        self.log.as_ref().map(|filename| self.create_log_file_(filename))
    }

    fn create_log_file_(&self, filename: &Option<String>) -> io::Result<File> {
        // Make sure the parent directories exist.
        fs::create_dir_all(&self.output_dir)?;

        let filename = filename.clone()
            // Use current date and time as default filename.
            .unwrap_or_else(|| format!("{}.log", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
        let log_path = self.output_dir.join(filename);

        // Do not buffer the log file, such that entries are immediately visible.
        File::create(log_path)
    }

    pub fn name_stats_file(&self) -> Option<io::Result<BufWriter<File>>> {
        if let Some(filename) = &self.type_save_name_stats {
            // See the help message above for the default filename.
            let filename = filename.as_deref().unwrap_or("name-stats.csv");
            let path = self.output_dir.join(filename);
            
            Some(File::create(path).map(BufWriter::new))
        } else {
            None
        }
    }

    pub fn keep_name_list(&self) -> Option<io::Result<Vec<Box<str>>>> {
        if let Some(path) = &self.type_keep_name_list {
            if self.type_remove_names {
                log::error!("conflicting options given: please only give either of --type-keep-name-list or --type-remove-names; ignoring the name list...");
            }
    
            Some(std::fs::read_to_string(path).map(|s| {
                let mut vec = Vec::new();
                for s in s.lines() {
                    vec.push(s.into())
                }
                vec
            }))
        } else {
            None
        }
    }

    pub fn wasm_repr(&self) -> anyhow::Result<WasmRepr> {
        Ok(match (self.wasm_repr, self.wasm_repr_size) {
            (WasmReprOption::Hash, None) => WasmRepr::Hash,
            (WasmReprOption::Full, None) => WasmRepr::Full,
            (WasmReprOption::Hash, Some(_))
            | (WasmReprOption::Full, Some(_)) => bail!("option --wasm-repr-size makes no sense with --wasm-repr 'hash' or 'full'"),
            (WasmReprOption::Subrange, Some(n)) => WasmRepr::Subrange(n),
            (WasmReprOption::Windows, Some(n)) => WasmRepr::Windows(n),
            (WasmReprOption::Subrange, None)
            | (WasmReprOption::Windows, None) => bail!("missing --wasm-repr-size=<N> for --wasm-repr 'subrange' or 'windows'"),
        })
    }
}

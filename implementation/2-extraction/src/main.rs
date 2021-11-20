#![allow(unused_imports)]
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::hash::Hash;
use std::io::Write;

use chashmap::CHashMap;
use clap::Clap;
use indicatif::{ParallelProgressIterator};
use itertools::Itertools;
use log::{Level, LevelFilter};
use options::Options;
use rand::prelude::*;
use rayon::iter::ParallelIterator;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use simplelog::{Color, CombinedLogger, LevelPadding, TermLogger, TerminalMode, WriteLogger};
use util::human_gnu_format;
use util::into_seq_iter::IntoSeqIter;

mod options;
mod samples;
mod util;
mod wasm;
use wasm::magic_bytes::is_wasm_by_magic_bytes;
use wasm::binary_stats::WasmBinaryStats;

use crate::options::Typedefs;
use crate::samples::sample::{WasmTypeSample, ParamOrReturn};
use util::frequencies::Frequencies;
use util::handle_errors::HandleErrorsIterExt;
use wasmparser::Operator;
use crate::samples::extract::extract_samples;
use crate::samples::types::{Type, TypeToken};
use crate::samples::wasm_repr::WasmRepr;
use crate::util::cmultimap::CMultiMap;
use crate::util::file_error::ResultWithFile;
use crate::util::handle_errors::HandleErrorsParIterExt;
use crate::wasm::fmt::type_str;
use crate::wasm::parse::WasmBody;
use util::ParallelProgressBar;
use util::percent::Percent;
mod dwarf;

fn main() -> anyhow::Result<()> {
    let options: Options = Options::parse();
    let wasm_repr = options.wasm_repr()?;

    // Print debug messages if verbose option is given.
    let log_level = if options.verbose { LevelFilter::Debug } else { LevelFilter::Info };

    let log_config = simplelog::ConfigBuilder::new()
        .set_time_to_local(true)
        .set_time_format_str("%F %T")
        .set_thread_level(LevelFilter::Off)
        .set_target_level(LevelFilter::Off)
        .set_level_color(Level::Info, Color::Green)
        .set_level_padding(LevelPadding::Right)
        .build();
    
    // Log additionally to a file, if that option is given.
    if let Some(log_file) = options.create_log_file() {
        CombinedLogger::init(vec![
            TermLogger::new(log_level, log_config.clone(), TerminalMode::Stdout), 
            WriteLogger::new(log_level, log_config, log_file?)
        ])
    } else {
        TermLogger::init(log_level, log_config, TerminalMode::Stdout)
    }?;

    log::debug!("built on {}", env!("BUILD_TIMESTAMP"));

    log::debug!("{:#?}", options);

    log::debug!("{} threads\n", rayon::current_num_threads());

    // Collect input files and all recursive files in input directories.
    let mut files: Vec<PathBuf> = options.input_files()
        .handle_errors(log_walkdir_error)
        .collect();
    // Sort files to make output deterministic.
    files.par_sort_unstable();

    log_number_human_aligned(files.len(), "input files found recursively");


    // Pass 1, over input files: Statistics for all binaries, compute signatures.

    let wasm_binaries_count = AtomicU64::new(0);

    let mut errors_magic_bytes = Vec::new();
    let mut errors_stats = Vec::new();

    let wasm_binaries_stats: Vec<(PathBuf, WasmBinaryStats)> = files
        .into_par_iter()

        // Show nice progress bar while reading in all files.
        .progress_bar()

        // Keep only Wasm binaries for further processesing.
        .filter_map(|file| 
            match is_wasm_by_magic_bytes(&file) {
                Ok(false) => None,
                Ok(true) => Some(Ok(file)),
                Err(err) => Some(Err(err)),
            }
        )

        // Collect errors into temporary vec and print after progress bar has finished, 
        // otherwise the console is cluttered with partial progress bar output.
        .collect_errors(&mut errors_magic_bytes)

        .inspect(|_| { wasm_binaries_count.fetch_add(1, Ordering::SeqCst); })

        .map(|file| WasmBinaryStats::from_file(&file).map(|stats| (file, stats)))

        .collect_errors(&mut errors_stats)

        .collect();

    for err in errors_magic_bytes {
        log::error!("{}: could not check for Wasm magic bytes, {}", err.file.display(), err.error);
    }
    log_number_human_aligned(wasm_binaries_count.into_inner(), "total Wasm binaries (by magic bytes)");

    for err in errors_stats {
        log::error!("{}: could not parse Wasm binary, {}", err.file.display(), err.error);
    }
    log_number_human_aligned(wasm_binaries_stats.len(), "total Wasm binaries (successfully parsed)\n");

    log::info!("stats on all (non-unique) parsed Wasm binaries:");

    let stats_total = wasm_binaries_stats
        .par_iter()
        .map(|(_, stats)| 
            (stats.file_size, stats.instruction_count, stats.function_bodies_count, stats.function_bodies_bytes)
        )
        .reduce(
            || (0, 0, 0, 0), 
            |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3));

    log_number_human_aligned(stats_total.1, "total instructions");
    log_number_human_aligned(stats_total.2, "total function bodies");
    log_filesize_human_aligned(stats_total.0, "total file size");
    log_filesize_human_aligned(stats_total.3, "total function bodies\n");
    
    let wasm_binaries_unique_sha256: HashSet<&[u8]> = wasm_binaries_stats
        .iter()
        .map(|(_, stats)| &stats.file_sha256[..])
        .collect();
    log_number_human_aligned(wasm_binaries_unique_sha256.len(), "unique Wasm binaries (by SHA256)");


    // Pass 2, over statistics of successfully parsed Wasm binaries: 
    // Remove duplicate binaries and report stats on the removed ones.
    
    // First sort (in parallel) including by filename to make the file order deterministic, 
    // and then remove consecutive duplicates.
    let mut wasm_binaries_stats = wasm_binaries_stats;
    wasm_binaries_stats.par_sort_unstable_by_key(|(path, stats)| (stats.binary_signature.clone(), path.clone()));
    
    // TODO deduplicate only when option is set, otherwise take files by SHA256.
    let mut wasm_binaries_unique_signature = 
        wasm_binaries_stats.iter()
        .dedup_by_with_count(|a, b| a.1.binary_signature == b.1.binary_signature)
        .collect_vec();
    log_number_human_aligned(wasm_binaries_unique_signature.len(), "unique Wasm binaries (by function signatures)\n");

    log::info!("most duplicated Wasm binaries (by function signatures):");

    wasm_binaries_unique_signature.par_sort_unstable_by_key(|(dup_count, _)| Reverse(*dup_count));
    for (dup_count, (file, stats)) in wasm_binaries_unique_signature.iter().take(options.stats_max) {
        if *dup_count > 1 {
            log::info!("{:6} x [example] {}", dup_count, file.display());
            log_number_human_aligned(stats.instruction_count, "total instructions");
            log_number_human_aligned(stats.function_bodies_count, "total function bodies");
        }
    }
    let duplication_factor = Percent::from_counts(wasm_binaries_stats.len() - wasm_binaries_unique_signature.len(), wasm_binaries_stats.len());
    log::info!("duplication factor: {}\n", duplication_factor);

    log::info!("stats on unique Wasm binaries (by function signatures):");

    let stats_unique = wasm_binaries_unique_signature
        .par_iter()
        .map(|(_, (_, stats))| 
            (stats.file_size, stats.instruction_count, stats.function_bodies_count, stats.function_bodies_bytes)
        )
        .reduce(
            || (0, 0, 0, 0), 
            |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3 + b.3));
    
    log_number_human_aligned(stats_unique.1, "total instructions");
    log_number_human_aligned(stats_unique.2, "total function bodies");
    log_filesize_human_aligned(stats_unique.0, "total file size");
    log_filesize_human_aligned(stats_unique.3, "total function bodies\n");
    

    // Pass 3, over unique Wasm binaries: extract samples.

    let rng = options.rng_with_seed();
    let wasm_add_raw_type = options.wasm_add_raw_type;   
    let (repr_desc, repr_fn): (String, Box<dyn Fn(&WasmTypeSample<WasmBody, Type>) -> anyhow::Result<WasmRepr> + Sync>) = match wasm_repr {
        options::WasmRepr::Hash => (
            "hash of full body bytes".to_string(),
            Box::new(|sample| Ok(WasmRepr::new_hash(sample, wasm_add_raw_type)))
        ),
        options::WasmRepr::Full => (
            "full body (but abstracted <param>)".to_string(),
            Box::new(|sample| WasmRepr::new_full(sample, wasm_add_raw_type))
        ),
        options::WasmRepr::Subrange(size) => (
            format!("(single) subrange with size {}", size),
            Box::new(move |sample| WasmRepr::new_subrange(sample, size, wasm_add_raw_type))
        ),
        options::WasmRepr::Windows(size) => (
            format!("(multiple) windows with size {}", size),
            Box::new(move |sample| WasmRepr::new_windows(sample, size, wasm_add_raw_type, &mut rng.clone()))
        ),
    };
    log::info!("input Wasm representation: {}\n", repr_desc);

    log::info!("extracting samples from binaries...");

    let mut errors_extraction_files = Vec::new();
    let mut errors_extraction_samples = Vec::new();

    let samples_removed_unused_param = AtomicU64::new(0);
    let samples_removed_unknown_type = AtomicU64::new(0);

    let name_stats = CMultiMap::new();
    let name_stats_file = options.name_stats_file().transpose()?;
    let keep_name_list = options.keep_name_list().transpose()?;

    let dataset_samples = wasm_binaries_unique_signature
        .into_par_iter()
        .progress_bar()

        // Parallel over binaries.
        .map(|(_count, (path, _stats))|  {
            
            // Makeshift try-block so that we can use ? inside.
            let result = (|| -> anyhow::Result<_> {

                let samples = 
                    // Parse WebAssembly binary and DWARF sections.
                    extract_samples(&path)?
                
                    // Filter out samples where the parameter is never used anywhere in the WebAssembly function.
                    .filter_ok(|sample| {
                        if !options.wasm_filter_unused_param {
                            return true;
                        }

                        match sample.param_or_return {
                            ParamOrReturn::Param { idx, .. } => {
                                let is_used = sample.wasm_body
                                    .instructions()
                                    .map(|instrs|
                                        instrs
                                            .filter_map(Result::ok)
                                            .any(|i| match i {
                                                Operator::LocalGet { local_index } 
                                                | Operator::LocalSet { local_index } 
                                                | Operator::LocalTee { local_index } if local_index == idx => true,
                                                _ => false
                                            })
                                    )
                                    .unwrap_or(false);

                                if !is_used {
                                    samples_removed_unused_param.fetch_add(1, Ordering::SeqCst);
                                }
                                is_used
                            }
                            ParamOrReturn::Return => true,
                        }
                    })

                    // Convert to own type language.
                    .map(|sample| -> anyhow::Result<_> {
                        let sample = sample?;
                        let ty = match sample.param_or_return {
                            ParamOrReturn::Param { .. } => Type::parse_param(&sample.type_),
                            ParamOrReturn::Return => Type::parse_type(&sample.type_),
                        }?;
                        Ok(sample.with_type(ty))
                    })

                    // // Statistics: non-determinism with full WebAssembly body.
                    // .map_ok(|sample| {
                    //     type_map_wasm_full.insert(WasmRepr::new_hash_str(&sample, true), &sample.type_);
                    //     sample
                    // })

                    // Convert to WebAssembly input representation.
                    .map(|sample| -> anyhow::Result<_> {
                        let sample = sample?;
                        let wasm_repr = repr_fn(&sample)?;

                        // // Statistics: non-determinism with our WebAssembly representation.
                        // type_map_wasm_repr.insert(wasm_repr.clone(), &sample.type_);

                        Ok(sample.with_wasm_body(wasm_repr))
                    })

                    // Attach file to error for better error reporting.
                    .map(|result| result.with_file(path.clone()))

                    // Filter out samples where the type is just Unknown.
                    .filter_ok(|sample| {
                        if !options.type_filter_unknown {
                            return true;
                        }

                        let is_unknown = sample.type_ == Type(vec![TypeToken::Unknown]);

                        if is_unknown {
                            samples_removed_unknown_type.fetch_add(1, Ordering::SeqCst);
                        }
                        !is_unknown
                    })

                    // Simplify types, if options given.
                    .map_ok(|mut sample| {
                        
                        // Collect statistics about names before removing or otherwise simplifying them.
                        if name_stats_file.is_some() {
                            for t in &sample.type_.0 {
                                match t {
                                    TypeToken::Typedef(name) | TypeToken::Nominal(name) => {
                                        name_stats.insert(name.clone(), &sample.file);
                                    }
                                    _ => {}
                                }
                            }
                        }

                        // Keep either no name at all, or only those in the given list.
                        if options.type_remove_names {
                            sample.type_.0.retain(|t| match t {
                                TypeToken::Typedef(_) | TypeToken::Nominal(_) => false,
                                _ => true
                            });
                        } else if let Some(keep) = &keep_name_list {
                            sample.type_.0.retain(|t| match t {
                                TypeToken::Typedef(name) | TypeToken::Nominal(name) => keep.contains(&name),
                                _ => true
                            });
                        }

                        match options.type_typedefs {
                            Typedefs::Keep => {}
                            Typedefs::ToNominal => {
                                for t in &mut sample.type_.0 {
                                    if let TypeToken::Typedef(name) = t {
                                        *t = TypeToken::Nominal(name.clone());
                                    }
                                }
                            }
                            Typedefs::Remove => {
                                sample.type_.0.retain(|t| if let TypeToken::Typedef(_) = t { false } else { true });
                            }
                        }

                        if options.type_name_flatten_outermost {
                            let mut outermost_name = false;
                            let mut new_type = Vec::with_capacity(sample.type_.0.len());
                            for t in sample.type_.0 {
                                new_type.push(match t {
                                    TypeToken::Typedef(_) | TypeToken::Nominal(_) if !outermost_name => {
                                        outermost_name = true;
                                        t
                                    },
                                    TypeToken::Typedef(_) | TypeToken::Nominal(_) => {
                                        continue
                                    }
                                    t => t
                                });
                            }
                            sample.type_.0 = new_type;
                        }

                        if options.type_remove_const {
                            sample.type_.0.retain(|t| t != &TypeToken::Const);
                        }

                        if options.type_class_to_struct {
                            for t in &mut sample.type_.0 {
                                if t == &TypeToken::Class {
                                    *t = TypeToken::Struct;
                                }
                            }
                        }
                        
                        sample
                    })

                    // Collect samples into Vec, for further parallel processing.
                    // (We cannot return the iterator directly here, because it contains ref-counted
                    // slices of the input files, which are not Send, which makes the iterator not Send, 
                    // and can thus it cannot be processed by rayon in parallel.
                    // An alternative would be to change all Rc -> Arc and remove collect_vec() below,
                    // but I am not sure which is more expensive: every ref-count being atomic when
                    // parsing the input vs. one single allocation more per binary. I strongly suspect
                    // the Arc'ing is more expensive. So that is why collect_vec().)
                    .collect_vec();
                Ok(samples)
            })();

            // Attach file to error for better reporting.
            result.with_file(path.clone())
        })

        .collect_errors(&mut errors_extraction_files)
        
        .flatten_iter()

        .collect_errors(&mut errors_extraction_samples);


    // Collect statistics on the samples (input/output tokens, unusual types).

    let types = CHashMap::new();
    let param_samples = AtomicU64::new(0);
    let return_samples = AtomicU64::new(0);

    // // Baseline mode: take most common output DWARF type for each input raw WebAssembly type.
    // // "Model" (i.e. mapping) extracted only on training data.
    // let baseline_model_train_params = CMultiMap::new();
    // let baseline_model_train_return = CMultiMap::new();

    // // "Perfect model": For each sample from the dev-set, just store the correct output for each input.
    // // However, due to non-determinism (same input, different outputs), there can be multiple
    // // output choices; then, take the best one.
    // let perfect_model_dev_params = CMultiMap::new();
    // let perfect_model_dev_return = CMultiMap::new();

    let dataset_samples = dataset_samples
        .inspect(|sample| {
            // Overall distribution of types.
            types.upsert(
                sample.type_.clone(),
                || 1,
                |count| *count += 1
            );

            match sample.param_or_return {
                ParamOrReturn::Param { .. } => param_samples.fetch_add(1, Ordering::SeqCst),
                ParamOrReturn::Return => return_samples.fetch_add(1, Ordering::SeqCst),
            };

            // // Simple Wasm type -> DWARF type, frequency-based baseline:
            // // "Build" model only on training data
            // if let TrainDevTest::Train = sample.aux {
            //     // Convert type first to string for printing it easily later.
            //     let type_str = type_str(sample.wasm_type);
            //     match sample.param_or_return {
            //         ParamOrReturn::Param { .. } => baseline_model_train_params.insert(type_str, &sample.type_),
            //         ParamOrReturn::Return => baseline_model_train_return.insert(type_str, &sample.type_)
            //     };
            // }

            // // "Perfect" model, if it could see all test data before.
            // if let TrainDevTest::Dev = sample.aux {
            //     // Do not store the complete input in the mapping but only a cyptographic hash, to save memory.
            //     let input_line = format!("{}", sample.wasm_body);
            //     let input_hash = format!("{:x}", Sha256::digest(input_line.as_bytes()));
            //     match sample.param_or_return {
            //         ParamOrReturn::Param { .. } => perfect_model_dev_params.insert(input_hash, &sample.type_),
            //         ParamOrReturn::Return => perfect_model_dev_return.insert(input_hash, &sample.type_)
            //     };
            // }
        });

    // Sequentially write output dataset for OpenNMT into text files.
    let mut sample_writer = options.sample_writer()?;

    for sample in dataset_samples.into_seq_iter() {
        sample_writer.write(&sample)?;
    }
    for err in errors_extraction_files.into_iter().sorted() {
        log::warn!("{}: could not extract samples, {}", err.file.display(), err.error);
    }
    for err in errors_extraction_samples.into_iter().sorted() {
        log::warn!("{}: could not extract samples, {}", err.file.display(), err.error);
    }

    log_number_human_aligned(samples_removed_unused_param.into_inner(), "samples removed because parameter was never used in WebAssembly function body");
    log_number_human_aligned(samples_removed_unknown_type.into_inner(), "samples removed where DWARF type was unknown\n");

    log::info!("samples total:");
    log_number_human_aligned(param_samples.into_inner(), "parameters");
    log_number_human_aligned(return_samples.into_inner(), "return values");

    log_filesize_human_aligned(sample_writer.bytes_written()?, "total bytes sample files written\n");

    // options.write_mapping_model(baseline_model_train_params, "param", "baseline-model-train")?;
    // options.write_mapping_model(baseline_model_train_return, "return", "baseline-model-train")?;

    // options.write_mapping_model(perfect_model_dev_params, "param", "perfect-model-dev")?;
    // options.write_mapping_model(perfect_model_dev_return, "return", "perfect-model-dev")?;

    // log_distribution(
    //     type_map_wasm_full.into_iter()
    //         .map(|(wasm, types)| (wasm, types.len() as u64)),
    //     "task inherent non-determinism, full Wasm instructions -> number of types",
    //     Some(options.stats_max));

    // log_distribution(
    //     type_map_wasm_repr.into_iter()
    //         .map(|(wasm, types)| (wasm, types.len() as u64)),
    //     "non-determinism with Wasm representation -> number of types",
    //     Some(options.stats_max));
    
    // log_distribution(
    //     frequency_baseline_wasm_raw_to_type.into_iter()
    //         .map(|(wasm_type, types)| wasm_type()), 
    //     "types", Some(options.stats_max));
    
    log_distribution(types, "types", Some(options.stats_max));

    if let Some(mut writer) = name_stats_file {
        writeln!(writer, "name,file,count")?;
        for (name, binaries) in name_stats {
            for (binary, count) in binaries {
                writeln!(writer, "\"{}\",\"{}\",{}", name, binary.display(), count)?;
            }
        }
    }

    // log::info!("results (on all data) for frequency-based baseline:");
    // let baseline = baseline.build();
    // log::info!("    overall accuracy: {}", baseline.accuracy());
    // use wasmparser::Type::*;
    // for wasm_type in [I32, I64, F32, F64].iter().copied() {
    //     if let Some(prediction) = baseline.prediction(wasm_type) {
    //         log::info!("    Wasm type {}:", type_str(wasm_type));
    //         log::info!("        prediction: {}", prediction.hl_type);
    //         log::info!("        accuracy:   {} ({}/{})", prediction.accuracy(), prediction.hl_type_correct, prediction.wasm_type_total);
    //         if wasm_type == F32 || wasm_type == F64 {
    //             log::info!("        all predictions: {}", baseline.all(wasm_type)
    //                 .map(|pred| format!("{} {}", pred.hl_type_correct, pred.hl_type))
    //                 .join(", "))
    //         }
    //     }
    // }

    // Output stats about DWARF tags encountered, extracted Wasm instructions, types etc.

    // let dwarf_entries_count = AtomicU64::new(0);
    // let dwarf_compilation_units_count = AtomicU64::new(0);
    // let dwarf_functions_count = AtomicU64::new(0);
    // let dwarf_functions_with_offset_count = AtomicU64::new(0);

    // let dwarf_wasm_functions_mappable_count = AtomicU64::new(0);
    // let dwarf_wasm_params_matching_len = AtomicU64::new(0);
    // let dwarf_wasm_param_count = AtomicU64::new(0);

    // let dwarf_entries_count = dwarf_entries_count.into_inner();
    // aligned_human_output(dwarf_entries_count, "DWARF entries total");
    // let dwarf_compilation_units_count = dwarf_compilation_units_count.into_inner();
    // aligned_human_output(dwarf_compilation_units_count, "DWARF compilation units total");
    // let dwarf_functions_count = dwarf_functions_count.into_inner();
    // aligned_human_output(dwarf_functions_count, "DWARF functions total");
    // let dwarf_functions_with_offset_count = dwarf_functions_with_offset_count.into_inner();
    // aligned_human_output(dwarf_functions_with_offset_count, "DWARF functions with offset");

    // let dwarf_wasm_functions_mappable_count = dwarf_wasm_functions_mappable_count.into_inner();
    // aligned_human_output(dwarf_wasm_functions_mappable_count, "DWARF unique functions that could be mapped to Wasm offset");
    // let dwarf_wasm_params_matching_len = dwarf_wasm_params_matching_len.into_inner();
    // aligned_human_output(dwarf_wasm_params_matching_len, "DWARF-Wasm unique functions where the parameter lengths match");
    // let dwarf_wasm_param_count = dwarf_wasm_param_count.into_inner();
    // aligned_human_output(dwarf_wasm_param_count, "DWARF-Wasm unique functions parameter count");

    // log::info!("primitive types (sorted by representation, {} unique):", types::parse_dwarf::PRIMITIVE_TYPE_COUNTS.len());
    // for ((ty, percent), count) in types::parse_dwarf::PRIMITIVE_TYPE_COUNTS
    //     .clone()
    //     .into_iter()
    //     .sorted_items()
    //     .with_percent()
    // {
    //     log::info!("{:11} ({:#}) {:22}  {:20}  bytes={:2}  =>  {}", count, percent, ty.source_name, ty.encoding, ty.byte_size, ty.normalized);
    // }

    // let samples_same_wasm_different_types: u64 = same_wasm_multiple_types.clone().into_iter().map(|(_, types)| types.len() as u64).sum();
    // log::info!(
    //     "Same Wasm, different types: {} ({})", 
    //     samples_same_wasm_different_types, 
    //     Percent::from_total(samples_same_wasm_different_types as f64, dataset_samples_counts.total() as f64)
    // );
    // for (wasm_hash, types) in same_wasm_multiple_types.clone().into_iter() {
    //     if types.len() > 1 {
    //         log::info!("{:x}: {} different types", wasm_hash, types.len());
    //         for ty in types {
    //             log::info!("    {}", ty);
    //         }
    //     }
    // }

    // log_distribution((*function_names).clone(), "function names");

    // log_distribution(types::parse_dwarf::TYPEDEF_NAME_COUNTS.clone(), "typedef names", Some(options.stats_max));
    // log_distribution(types::parse_dwarf::NOMINAL_NAME_COUNTS.clone(), "nominal type names", Some(options.stats_max));

    // log_distribution(dwarf::UNSPECIFIED_NAME_COUNTS.clone(), "unspecified type names");
    // log_distribution((*unspecified_sources).clone(), "unspecified type sources");

    // log_distribution(dwarf::UNKNOWN_NAME_COUNTS.clone(), "unknown type names");
    // log_distribution((*unknown_sources).clone(), "unknown type sources");

    // log::info!("average input Wasm token sequence length: {:.2}", (*wasm_tokens).iter().map(|(_tok, count)| count).sum::<u64>() as f64 / dataset_samples_counts.total() as f64);
    // log_distribution(wasm_tokens.clone(), "input Wasm tokens");

    // log::info!("average output type token sequence length: {:.2}", type_tokens.iter().map(|(_tok, count)| count).sum::<u64>() as f64 / dataset_samples_counts.total() as f64);
    // log_distribution((*type_tokens).clone(), "output type tokens");

    // let names_across_binaries = names
    //     .into_iter()
    //     .map(|(name, binaries)| {
    //         let binaries_count = binaries.len() as u64;
    //         // HACK put percent in item, just for quick printing
    //         let name_with_binary_percent = format!(
    //             "{:20} ({} of all binaries)", 
    //             name, 
    //             Percent::from_counts(binaries_count, dataset_binaries_total)
    //         );
    //         (name_with_binary_percent, binaries_count)
    //     });
    // log_distribution(names_across_binaries, "nominal type names (counted once per binary)", Some(options.stats_max));

    // log_distribution(types, "types", Some(options.stats_max));
    Ok(())
}

fn log_number_human_aligned(uint: impl TryInto<u64>, description: &str) {
    let uint: u64 = uint.try_into().ok().unwrap();
    log::info!("{:11} ({:>4}) {}", uint, human_gnu_format::format_integer(uint), description);
}

fn log_filesize_human_aligned(uint: u64, description: &str) {
    log::info!("{:11} ({:>6}) {}", uint, human_gnu_format::format_file_size_binary(uint), description);
}

fn log_distribution<T, I>(counts: I, description: &str, n: Option<usize>) 
where 
    T: Display + Clone + Hash + Eq,
    I: IntoIterator<Item = (T, u64)>,
{
    let counts: Vec<(_, _)> = counts.into_iter().collect();

    log::info!(
        "{}, {} total, {}{} unique:", 
        description, 
        counts.iter().map(|(item, count)| (item, *count)).total_count(),
        if let Some(n) = n {
            format!("most common {} of ", n)
        } else {
            String::new()
        },
        counts.len()
    );
    for ((item, percent), count) in counts
        .iter()
        .map(|(item, count)| (item, *count))
        .with_percent()
        .most_common(n.unwrap_or(counts.len()))
    {
        log::info!("{:11} ({:#}) {}", count, percent, item);
    }
}

fn log_walkdir_error(err: walkdir::Error) {
    match (err.path(), err.io_error()) {
        (Some(path), Some(io_err)) => log::error!("{}: {}", path.display(), io_err),
        (Some(path), None) => log::error!("{}: {}", path.display(), err),
        (None, _) => log::error!("{}", err),
    }
}

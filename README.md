# Finding the Dwarf: Recovering Precise Types from WebAssembly Binaries

This is the supplementary material for `SnowWhite`, a type prediction system as described in the paper "Finding the Dwarf: Recovering Precise Types from WebAssembly Binaries" (PLDI 2022).

It contains the source code, input dataset, trained models, and final evaluation results to help others understand, replicate, and extend our work.
Please open an issue in case you have a question or trouble executing the steps below.
There currently is no Docker image or one-click script to run all the steps below. 

## Setup and Requirements

We tested and developed the system on Ubuntu 18.04 LTS with:

- Python 3.8 with `numpy` and `pandas`, and Bash for general scripting.
- 7z/p7zip: for extracting the compressed datasets.
- [Jupyter Notebook](https://jupyter.org/install): for quick data inspection, extracting common name list.
- [OpenNMT-py 2.0](https://opennmt.net/OpenNMT-py/main.html): for implementation of the sequence-to-sequence models themselves.
- [SentencePiece](https://github.com/google/sentencepiece): for subword models for WebAssembly and type sequences.
- [Rust](https://www.rust-lang.org/) distribution with compiler and `cargo` package manager: for parsing WebAssembly and DWARF information and training data extraction.
- [Emscripten](https://emscripten.org/): for compiling C/C++ code to WebAssembly with DWARF info, ideally install into `~/emsdk/`.
- [LLVM](https://llvm.org) (optional): for `llvm-dwarfdump` to inspect DWARF information in WebAssembly `.o` files.
- [WABT (WebAssembly Binary Toolkit)](https://github.com/webassembly/wabt/) (optional): for `wasm-objdump` and `wasm2wat` to inspect WebAssembly binaries.

The linked repositories / pages contain the most up-to-date installation instructions, so follow those.

## Files, Directory Structure

This repository is roughly organized into the following directories and files:

- `data/`: The most important input and intermediate files of the approach. Notably:
    * `ubuntu-packages.txt`: List of all Ubuntu packages that were attempted to be compiled to WebAssembly.
    * `binaries.7z`: All successfully compiled object files that contained WebAssembly code and DWARF debug information.
    * `name-stats.csv`: Extracted statistics on all type names (in typedefs and named datatypes).
    * `common-names.txt`: List of type names that appear at least once in 1% or more of all packages.
    * `dataset.7z`: Split and deduplicated training/dev/test textual data as input to sequence-to-sequence neural network.
- `implementation/`: Source code of the data collection, extraction of training data, various scripts for data splitting, and shell scripts for training and evaluating the models. See below for more information about the pipeline.
- `models/`: Trained models on our data, such that if you don't want to run the whole data extraction, training etc., you can just use those saved model checkpoints.
    * `subword/`: SentencePiece subword models for WebAssembly and type tokens.
    * `seq2seq/`: OpenNMT-py 2.0 sequence-to-sequence neural model. Config files used for `onmt_build_vocab` and `onmt_train` and saved model parameters checkpoint of the best model on the dev dataset.
- `results-testdata/`: Predictions of both the statistical baseline model (see paper) and our best model, for each combination of the five type languages (see paper) and param/return type prediction.
    * See the test set in `dataset.7z` for the respective WebAssembly input sequences. 
    * `predictions.*.txt`: Since we use beam search with n=5 beams, there are n times as many predictions as there are input sequences.
    * `*.eval.log`: Output of our final evaluation script with measures such as top-k exact match accuracy, type prefix score, BLEU etc.
- **Compressed data**: A lot of the data compresses extraordinarily well, e.g., the raw WebAssembly binaries (90 -> 6GB) and textual training data (20GB -> 2GB). We compressed them with `7z a -t7z -m0=lzma2 -mx=9 <compressed.7z> <input>`. Those files end in `.7z`. Make sure there is enough disk space available before unpacking.
- **Externally hosted files**: Very large files, e.g., the archives of all compiled binaries and pre-split train/dev/test set sequences are not committed to Git to not blow up the repository size, but instead hosted on a file hoster and linked via `.link` files (poor man's Git LFS).

## Steps, Code Components, Inputs / Outputs

The high-level steps for running the project are (including training, not just inference, ommitting some details for brevity):

1. **Collect binaries**: Compile (a filtered subset of) all Ubuntu source packages with Emscripten to WebAssembly with DWARF debug information.
    * Input: list of packages to download and compile.
    * Output: the `binaries.7z` dataset, i.e., roughly 90GB of WebAssembly object files (and 400GB of source files, not contained).
    * Required Resources: high-bandwidth Internet connection, runtime is days to weeks (but it is trivially parallelizable), ~500GB hard drive space, ideally all inside a VM/Docker container.
1. **Extract samples**: Parse WebAssembly and DWARF, deduplicate, convert to textual data for neural model.
    * Input: binaries from above, extraction options (e.g., how to simplify, which names to keep, see below).
    * Output: 2 (parameter and return samples) * 3 files (WebAssembly and type sequences, metadata to map samples back to source files), which are ~20 GB text files.
    * Required Resources: >16GB memory, ideally high threadcount CPU, runtime is minutes to an hour (with a warm page cache).
1. **Split dataset, inspect data, collect statistics**: Various scripts to split the dataset into train/dev/test sets, gather the distribution of types, and select a subset of common type names (Jupyter notebook). (Go back to step 3 for extracting types only with `common-names.txt`.)
    * Input: 3 text files per parameter/return dataset portion.
    * Output: the `dataset.7z` dataset, i.e., 4 (train/dev/dev.subsample/train sets) * 2 (param/return) * 3 text files, and the `common-names.txt` file.
    * Required Resources: >32GB memory, runtime is minutes to an hour.
1. **Train subword models**: Train SentencePiece models for WebAssembly and type tokens (except for Eklavya and simple type language without names, where the vocabulary is already small enough).
    * Input: input/output text sequences from training data, merged from parameter and return portions.
    * Output: subword models, i.e., `.vocab` and `.model` files.
    * Required Resources: >32GB memory, runtime is (low number of) hours.
1. **Train neural seq2seq models**: `onmt_build_vocab` and `onmt_train` from OpenNMT-py 2.0.
    * Input: training data text sequences, subword models, OpenNMT config file.
    * Output: model checkpoints, training and validation accuracy over time.
    * Required Resources: (ideally multiple) GPUs, runtime is (high number of) hours to several days.
1. (Play with type languages, hyperparameters, model architectures etc.)
    * Required Resources: days to months of training time.
1. **Predict types with model**: `onmt_translate` on WebAssembly text inputs (and translate and back-translate tokens with subword model).
    * Input: input WebAssembly sequences, from validation data (if you still develop the model) or test data (if you are certain you won't change anything anymore); neural model checkpoint; subword models.
    * Output: predicted type sequences.
    * Required Resources: depending on the number of test samples, for us (~200-500k test data samples) hours, on a GPU.
1. **Evaluate, compare against baselines, etc.**: Get metrics like top-k accuracy, type prefix score, BLEU score, most common mispredictions etc. from `scripts/evaluate-predictions.py`.
    * Input: predictions from the model or baseline models, and ground-truth data.
    * Output: don't trust a single number, also inspect the individual predictions! See `results-testdata/`.

A lot of those steps are not automated; especially since training and evaluation require one-off command-line program invocations.
If you feel something should be automated, but isn't open an issue or PR.
Otherwise, many invocations are documented in `notebook.sh` files, which are basically commented Bash `history`.

## License and Warranty

For our own code: MIT, see `LICENSE` file. The respective licenses for other code.

Obviously, as a research project, this code comes with **ABSOLUTELY NO WARRANTY**. Especially the building of tens of thousands of binaries from arbitrary source code may **brick your system, delete your hard drive, upload all your data, or do anything else unexpected** (though we don't hope it does).
Finally, working with data of this size requires some **beefy hardware in terms of hard drive storage space, high thread-count CPU, lots of RAM, and high-memory GPU** for neural network training.
See above for back-of-the-envolope estimates of the runtimes and hardware requirements.
**Running any of this on a "commodity laptop" will most likely make your machine run out of disk space or RAM, freeze the system, over exert the physical cooling capacity, and even damage the machine permanently, without ever producing meaningful results**.
An appropriate machine has _at least_ 8 cores, 32GB RAM, 1TB free disk space, a fast GPU, and >150W cooling capacity.
We ran it on a server with 48 cores, 256GB RAM, 6TB SSD, 2 GPUs, and >500W power draw.

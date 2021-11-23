#!/bin/bash

# Merge parameter and return vocabularies together for WebAssembly
mkdir wasm
pushd wasm
cat ~/wasm-type-prediction/data/extracted/final-eval/names-filtered/split-by-dir-shuffle/*/train/wasm.txt > train.txt
wc train.txt

# One shared WebAssembly vocabulary for all models, also parameter and return together
~/wasm-type-prediction/sentencepiece/build/src/spm_train \
    --input='train.txt' \
    --model_prefix='500' \
    --vocab_size=500 \
    --model_type=bpe \
    --character_coverage=1.0 \
    --user_defined_symbols='<param>,<begin>,<window>,=' \
    --bos_id=-1 \
    --eos_id=-1 \
    --train_extremely_large_corpus=true \
    --max_sentence_length=10000

popd

# One vocabulary for each type language variant
# but not for no-names-class-const and eklavya because there are too little distinct tokens
for ty in names-all names-filtered
do
    mkdir -p type/$ty/
    pushd type/$ty/
    cat ~/wasm-type-prediction/data/extracted/final-eval/$ty/split-by-dir-shuffle/*/train/type.txt > train.txt
    wc train.txt

    ~/wasm-type-prediction/sentencepiece/build/src/spm_train \
        --input='train.txt' \
        --model_prefix='500' \
        --vocab_size=500 \
        --model_type=bpe \
        --character_coverage=1.0 \
        --bos_id=-1 \
        --eos_id=-1 \
        --train_extremely_large_corpus=true \
        --max_sentence_length=10000
    popd
done

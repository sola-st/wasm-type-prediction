#!/bin/bash

VARIANTS=$(ls ~/wasm-type-prediction/data/extracted/final-eval | grep -v '\.')
for variant in $VARIANTS
do
    mkdir -p model/$variant/{param,return}
done

# TODO iterate over all model configurations

# TODO copy and adapt config.yaml file
echo copy config from previous project
# - no tgt_subword_type and _model for eklavya and _kwargs for
# - switch between gpu 0 and 1
# - paths to subword models
# - paths to data

# For samples to inspect
onmt_build_vocab --config config.yaml --n_sample=100 --dump_samples
# Build vocab of full training data
onmt_build_vocab --config config.yaml --n_sample=-1 --num_threads=10 --overwrite

# TODO training of the remaining param models
for variant in eklavya no-names-class-const names-filtered-no-wasm-ty
do
    pushd "$variant/param"
    # add this line if you want to train on the second GPU
    # CUDA_VISIBLE_DEVICES=1 \
        onmt_train --config config.yaml
    popd
done

# evaluate on test data
# take best model on dev set
ln -s "model_step_$(rg '.* Best model found at step (\d+)' train.log -r '$1' | tail -n1).pt" model_best.pt
ls -lt

# Change for each model variant
variant='names-filtered-no-wasm-ty'
por='param'
type_spm='.spm' # or '' if types have no SPM model
# type_spm='' # or '' if types have no SPM model
# Encode input with subword model.
~/wasm-type-prediction/sentencepiece/build/src/spm_encode \
    --model='../../../subword-model/wasm/500.model' \
    < "/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/wasm.txt" \
    > 'test.wasm.spm.txt'
# Then translate
CUDA_VISIBLE_DEVICES=1 \
onmt_translate \
    --src='test.wasm.spm.txt' \
    --model='model_best.pt' \
    --output="predictions.model_best$type_spm.txt" \
    --log_file='predict.log' \
    --n_best=5 \
    --beam_size=5 \
    --report_time \
    --gpu=0 \
    --batch_size=100
# Use SentencePiece to decode the output back into regular tokens again, but only for models where the types are in a subword model.
if test -n "$type_spm"
then
    ~/wasm-type-prediction/sentencepiece/build/src/spm_decode \
        --model="../../../subword-model/type/$variant/500.model" \
        < 'predictions.model_best.spm.txt' \
        > 'predictions.model_best.txt'
fi
# Evaluate against ground-truth
~/wasm-type-prediction/scripts/evaluate-predictions.py \
    --log="eval.log" \
    --predictions="predictions.model_best.txt" \
    --ground-truth="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/type.txt"



# BASELINE MODEL, FOR COMPARISON

VARIANTS=$(ls ~/wasm-type-prediction/data/extracted/final-eval | grep -v '\.')
for variant in $VARIANTS
do
    for por in return param
    do
        pushd "$variant/$por"

        # # Baseline: type distribution
        # echo 'type distribution:'
        # ~/wasm-type-prediction/scripts/types-distribution.py \
        #     --logfile 'type-distribution.test.log' \
        #     --types "/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/type.txt"
        # # Simple baseline: raw Wasm type -> source type, based on conditional output type distribution
        # echo 'baseline train:'
        # ~/wasm-type-prediction/scripts/baseline-model-build.py \
        #     --wasm="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/train/wasm.txt" \
        #     --types="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/train/type.txt" \
        #     --out='baseline-model.model.json' \
        #     --logfile='baseline-model.train.log'
        # echo 'baseline predict:'
        # ~/wasm-type-prediction/scripts/baseline-model-predict.py \
        #     --wasm="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/wasm.txt" \
        #     --model='baseline-model.model.json' \
        #     --out='predictions.baseline-model.txt' \
        #     --top-k=5 \
        #     --logfile='baseline-model.predict.log'
        echo 'baseline eval:'
        ~/wasm-type-prediction/scripts/evaluate-predictions.py \
            --predictions='predictions.baseline-model.txt' \
            --ground-truth="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/type.txt" \
            --log='baseline-model.eval.log'
        echo 'predictions eval:'
        ~/wasm-type-prediction/scripts/evaluate-predictions.py \
            --log="eval.log" \
            --predictions="predictions.model_best.txt" \
            --ground-truth="/home/daniel/wasm-type-prediction/data/extracted/final-eval/$variant/split-by-dir-shuffle/$por/test/type.txt"


        popd
    done
done

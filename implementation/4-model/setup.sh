#!/bin/sh

python3 -m venv .
source bin/activate

# see https://pypi.org/project/OpenNMT-py/
pip install OpenNMT-py
pip install nltk

mkdir -p tutorial
pushd tutorial

wget https://s3.amazonaws.com/opennmt-trainingdata/toy-ende.tar.gz
tar xf toy-ende.tar.gz

head -n 2 toy-ende/src-train.txt

onmt_build_vocab -config config.yaml -n_sample 10000
onmt_train -config config.yaml
onmt_translate -model toy-ende/run/model_step_1000.pt -src toy-ende/src-test.txt -output toy-ende/pred_1000.txt -gpu 0 -verbose

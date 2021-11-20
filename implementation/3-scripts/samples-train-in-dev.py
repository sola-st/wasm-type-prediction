#!/usr/bin/env python3

import argparse
import logging
import sys
import pathlib

parser = argparse.ArgumentParser()
parser.add_argument('--dataset', metavar='<path>', type=pathlib.Path, required=True)
# parser.add_argument('--train-wasm', '-tw', metavar='<file>', required=True)
# parser.add_argument('--train-types', '-tt', metavar='<file>', required=True)
# parser.add_argument('--dev-wasm', '-dw', metavar='<file>', required=True)
# parser.add_argument('--dev-types', '-dt', metavar='<file>', required=True)
parser.add_argument('--verbose', '-v', default=False, action='store_true', help='produce debug output')
parser.add_argument('--logfile', metavar='<file>', help='write console output also to logfile (default: log only to console)', default=None)
args = parser.parse_args()

log_handlers = [logging.StreamHandler(sys.stdout)]
if args.logfile:
    log_handlers.append(logging.FileHandler(args.logfile))
logging.basicConfig(
    format='%(asctime)s %(levelname)-8s %(message)s', 
    level=logging.DEBUG if args.verbose else logging.INFO,
    datefmt='%Y-%m-%d %H:%M:%S',
    handlers=log_handlers
)
log = logging.getLogger(__name__)

log.debug(args)

train_wasm = args.dataset / 'train' / 'wasm.txt'
train_types = args.dataset / 'train' / 'type.txt'
dev_wasm = args.dataset / 'dev' / 'wasm.txt'
dev_types = args.dataset / 'dev' / 'type.txt'

log.info('reading train files...')
with open(train_wasm) as f:
    wasm = [line.strip() for line in f.readlines()]
with open(train_types) as f:
    types = [line.strip() for line in f.readlines()]
assert len(wasm) == len(types)
train_samples = len(wasm)
log.info(f'train samples: {train_samples}')

log.info('building train sets...')
train_both = set(zip(wasm, types))
train_wasm = set(wasm)

log.info('reading dev files...')
with open(dev_wasm) as f:
    wasm = [line.strip() for line in f.readlines()]
with open(dev_types) as f:
    types = [line.strip() for line in f.readlines()]
assert len(wasm) == len(types)
dev_samples = len(wasm)
log.info(f'dev samples: {dev_samples}')

log.info('checking dev samples in train sets...')
dev_train_duplicate_both = 0
dev_train_duplicate_wasm = 0
for w, t in zip(wasm, types):
    if (w, t) in train_both:
        dev_train_duplicate_both += 1
    if w in train_wasm:
        dev_train_duplicate_wasm += 1

log.info(f'dev samples fully in training data:     {dev_train_duplicate_both:10} ({dev_train_duplicate_both/dev_samples:.2%})')
log.info(f'dev sample wasm input in training data: {dev_train_duplicate_wasm:10} ({dev_train_duplicate_wasm/dev_samples:.2%})')

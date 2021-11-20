#!/usr/bin/env python3

import argparse
import logging
import sys

from collections import Counter

parser = argparse.ArgumentParser()
parser.add_argument('--wasm', '-w', metavar='<file>', required=True)
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

log.info('reading input file...')
with open(args.wasm) as f:
    wasm = [line.strip() for line in f.readlines()]
n_samples = len(wasm)
log.info(f'samples: {n_samples}')

log.info('counting Wasm bodies...')
wasm_counts = Counter()
for w in wasm:
    wasm_counts[w] += 1

log.info('Wasm bodies with the most samples:')
n_duplicated_wasm_samples = 0
for wasm, sample_count in wasm_counts.most_common(100):
    log.info(f'{sample_count:8} ({sample_count/n_samples:6.2%}) {wasm}')
    if sample_count > 1:
        n_duplicated_wasm_samples += sample_count

log.info(f'duplicated Wasm body samples: {n_duplicated_wasm_samples} ({n_duplicated_wasm_samples/n_samples:.2%})')

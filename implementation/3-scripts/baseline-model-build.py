#!/usr/bin/env python3

import argparse
import logging
import sys
import json

from collections import Counter, defaultdict

parser = argparse.ArgumentParser()
parser.add_argument('--wasm', '-w', metavar='<file>', required=True)
parser.add_argument('--types', '-t', metavar='<file>', required=True)
parser.add_argument('--out', '-o', metavar='<file>')
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

log.info('reading input files...')
with open(args.wasm) as f:
    # The first token in each line is the WebAssembly raw type.
    wasm = [line.strip().split()[0] for line in f.readlines()]
with open(args.types) as f:
    types = [line.strip() for line in f.readlines()]

assert len(wasm) == len(types)
n_samples = len(wasm)
log.info(f'samples: {n_samples}')

log.info('building map: wasm -> {type: count}...')
wasm_to_types_map = defaultdict(Counter)
for wasm, ty in zip(wasm, types):
    wasm_to_types_map[wasm][ty] += 1

log.info('model:')
model = {}
for wasm, type_counts in wasm_to_types_map.items():
    types = [ty for ty, count in type_counts.most_common()]
    model[wasm] = types
    total_count = sum(type_counts.values())

    log.info(f'{total_count:8} {wasm}')
    for ty, count in type_counts.most_common(20):
        log.info(f'  {count:8} ({count/total_count:7.2%}) {ty}')

if args.out:
    with open(args.out, 'w') as f:
        json.dump(model, f)

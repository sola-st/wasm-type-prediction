#!/usr/bin/env python3

import argparse
import logging
import sys
import json
import hashlib

parser = argparse.ArgumentParser()
parser.add_argument('--wasm', '-w', metavar='<file>', required=True)
parser.add_argument('--model', '-m', metavar='<file>', required=True)
parser.add_argument('--out', '-o', metavar='<file>', required=True)
parser.add_argument('--top-k', '-k', metavar='N', required=True)
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

def hash_str(str) -> str:
    return hashlib.sha256(str.encode('utf-8')).hexdigest()

log.info('reading input files...')
with open(args.wasm) as f:
    # Use same hasing of inputs as for building the model.
    wasm = [hash_str(line.strip()) for line in f.readlines()]
n_samples = len(wasm)
log.info(f'samples: {n_samples}')

with open(args.model) as f:
    model = json.load(f)
log.info(f'model keys: {len(model)}')

top_k = int(args.top_k)
log.info(f'top k: {top_k}')

log.info('predicting...')
with open(args.out, 'w') as f:
    for w in wasm:
        types = model.get(w, ['<unknown>'])
        # At most k predictions:
        types = types[:top_k]
        # At least k predictions:
        # https://stackoverflow.com/questions/3438756/some-built-in-to-pad-a-list-in-python
        types += ['<pad>'] * (top_k - len(types))
        for ty in types:
            f.write(f'{ty}\n')

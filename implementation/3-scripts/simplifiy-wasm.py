#!/usr/bin/env python3

import argparse
import logging
import sys
import re

from collections import Counter

parser = argparse.ArgumentParser()
parser.add_argument('--wasm', '-w', metavar='<file>', required=True)
# parser.add_argument('--mode', choices=['classes', 'eklavya'], required=True)
parser.add_argument('--out', '-o', metavar='<file>', required=True)
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

log.info('simplifiying...')
simplified = []
for w in wasm:
    simplified.append(re.sub('(\s+-?|=)\d+', '', w))
    # TODO Alternatives:
    # - replace numbers with <num> token, to eliminate effect on token sequence length.
    # - replace only numbers after .const instructions, but keep indices of local.*, global.* etc.

log.info(f"writing '{args.out}'...")
with open(args.out, 'w') as f:
    for sample in simplified:
        f.write(f'{sample}\n')

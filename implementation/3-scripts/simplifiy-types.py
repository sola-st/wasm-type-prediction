#!/usr/bin/env python3

import argparse
import logging
import sys
import re

from collections import Counter

parser = argparse.ArgumentParser()
parser.add_argument('--types', '-t', metavar='<file>', required=True)
parser.add_argument('--mode', choices=['classes', 'eklavya'], required=True)
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
with open(args.types) as f:
    types = [line.strip() for line in f.readlines()]
n_samples = len(types)
log.info(f'samples: {n_samples}')


log.info('simplifiying...')
simplified = []
type_counts = Counter()
for ty in types:
    if args.mode == 'classes':
        ty_simplified = ty.replace(" ", "_")
    elif args.mode == 'eklavya':
        # From the paper: τ ::= int|char|float|void∗|enum|union|struct
        ty_simplified = ' '.join(tt for tt in ty.split() if tt != 'const')
        if ty_simplified.startswith('pointer'):
            ty_simplified = 'pointer'
        elif ty_simplified.startswith('enum'):
            ty_simplified = 'enum'
        elif ty_simplified.startswith('primitive float') or ty_simplified == 'primitive complex':
            ty_simplified = 'float'
        elif ty_simplified.startswith('primitive uint') or ty_simplified.startswith('primitive int') or ty_simplified == 'primitive bool':
            ty_simplified = 'int'
        elif ty_simplified.startswith('primitive char'):
            ty_simplified = 'char'
        elif ty_simplified == 'union':
            ty_simplified = 'union'
        elif ty_simplified == 'class' or ty_simplified == 'struct':
            ty_simplified = 'struct'
        else:
            raise NotImplementedError(ty, ty_simplified)
    else:
        raise NotImplementedError(args.mode)
    # type_counts[f'{ty} -> {ty_simplified}'] += 1
    type_counts[ty_simplified] += 1
    simplified.append(ty_simplified)

log.info(f'{len(type_counts)} unique types:')
for ty, count in type_counts.most_common(100):
    log.info(f'{count:8} ({count/n_samples:6.2%}) {ty}')

log.info(f"writing '{args.out}'...")
with open(args.out, 'w') as f:
    for sample in simplified:
        f.write(f'{sample}\n')

    
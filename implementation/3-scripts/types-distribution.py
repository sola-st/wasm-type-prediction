#!/usr/bin/env python3

import argparse
import logging
import sys

from collections import Counter

parser = argparse.ArgumentParser()
parser.add_argument('--types', '-t', metavar='<file>', required=True)
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

log.info('counting types...')
type_counts = Counter()
for t in types:
    type_counts[t] += 1

log.info('most common types:')
for ty, count in type_counts.most_common(100):
    log.info(f'{count:8} ({count/n_samples:6.2%}) {ty}')

#!/usr/bin/env python3

import argparse
import logging
import sys
import json

from collections import Counter

parser = argparse.ArgumentParser()
parser.add_argument('--info-file', '-i', metavar='<file>', required=True)
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
with open(args.info_file) as f:
    sample_binaries = [json.loads(line.strip())['file'] for line in f.readlines()]
log.info(f'samples: {len(sample_binaries)}')

log.info('counting binaries...')
binaries = Counter()
for binary in sample_binaries:
    binaries[binary] += 1

log.info('binaries with the most samples:')
for binary, sample_count in binaries.most_common(100):
    log.info(f'{sample_count:8} ({sample_count/len(sample_binaries):6.2%}) {binary}')

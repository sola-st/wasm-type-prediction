#!/usr/bin/env python3

import argparse
import logging
import sys
import pandas as pd

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

def parse_wasm(wasm: str) -> [[str]]:
    splitted = wasm.split('<begin>')
    if len(splitted) == 2:
        rest = splitted[1]
    # allow empty lines for debugging
    # elif len(splitted) == 1 and splitted[0] == '':
    #    return []
    else:
        raise ValueError(f"unexpected wasm format, expected exactly one <begin> token, got: '{wasm}'")
    windows = rest.split('<window>')
    windows = [[i.strip() for i in window.split(';')] for window in windows]
    return windows

def token_count(wasm: str) -> int:
    return len(wasm.split())

def window_count(parsed_wasm: [[str]]) -> int:
    return len(parsed_wasm)

def instr_count(parsed_wasm: [[str]]) -> int:
    return sum(len(window) for window in parsed_wasm)

def pad_count(parsed_wasm: [[str]]) -> int:
    return sum(window.count('<pad>') for window in parsed_wasm)

log.info('gathering statistics...')
stats = []
for w in wasm:
    # log.debug(w)
    parsed_wasm = parse_wasm(w)
    instr = instr_count(parsed_wasm)
    pad = pad_count(parsed_wasm)
    stats.append({
        'token_count': token_count(w),
        'window_count': window_count(parsed_wasm),
        'instr_count': instr,
        'pad_count': pad,
        'pad_percent': pad/instr*100,
    })

stats = pd.DataFrame(stats)
pd.set_option('display.float_format', lambda x: '%.2f' % x)
log.info(f'overview stats:\n{stats.describe(percentiles=[.5,.75,.9,.95,.99,.999])}')
log.info(f'sums:\n{stats.sum()}')

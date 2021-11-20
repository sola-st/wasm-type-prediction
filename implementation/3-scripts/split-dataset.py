#!/usr/bin/env python3

import argparse
import logging
import sys
import random
import pathlib
import json
import os

from collections import Counter, defaultdict

def parse_split_ratios(str: str) -> [float, float, float]:
    ratios = [float(x) for x in str.split('/')]
    [train, dev, test] = ratios

    total = sum(ratios)
    assert total > 0
    return train/total, dev/total, test/total

parser = argparse.ArgumentParser()
parser.add_argument('--dataset', '-d', metavar='<path>', type=pathlib.Path, required=True)
parser.add_argument('--out', '-o', metavar='<path>', type=pathlib.Path, default='.')
parser.add_argument('--by', '-b', choices=['function', 'binary', 'dir'], required=True)
parser.add_argument('--ratio', '-r', metavar='<train/dev/test>', type=parse_split_ratios, required=True)
parser.add_argument('--limit-samples', default=False, action='store_true', help='limit samples of most common class to at most the ones of the second most common')
parser.add_argument('--dev-subsamples', metavar='N', type=int, default=None)
parser.add_argument('--seed', metavar='N', type=int, default=0)
parser.add_argument('--shuffle', default=False, action='store_true', help='shuffle samples before output')
parser.add_argument('--verbose', '-v', default=False, action='store_true', help='produce debug output')
parser.add_argument('--logfile', '-l', metavar='<file>', help='write console output also to logfile (default: log only to console)', default=None)
args = parser.parse_args()

os.makedirs(args.out, exist_ok=True)

log_handlers = [logging.StreamHandler(sys.stdout)]
if args.logfile:
    log_handlers.append(logging.FileHandler(args.out / args.logfile))
logging.basicConfig(
    format='%(asctime)s %(levelname)-8s %(message)s', 
    level=logging.DEBUG if args.verbose else logging.INFO,
    datefmt='%Y-%m-%d %H:%M:%S',
    handlers=log_handlers
)
log = logging.getLogger(__name__)

log.debug(args)


log.info('reading dataset...')
with open(args.dataset / 'info.jsonl') as f:
    info = [json.loads(line.strip()) for line in f.readlines()]
with open(args.dataset / 'wasm.txt') as f:
    wasm = [line.strip() for line in f.readlines()]
with open(args.dataset / 'type.txt') as f:
    types = [line.strip() for line in f.readlines()]

assert len(info) == len(wasm) == len(types)
n_samples = len(wasm)
log.info(f'  samples: {n_samples}')


def function_id(info):
    return f"{info['file']} $f{info['function_idx']}"

def binary(info):
    return info['file']

log.info('common directory of all samples...')
commonpath = os.path.commonpath(i['file'] for i in info)
log.info(f"  '{commonpath}'")

def first_dir(info):
    path_relative = os.path.relpath(info['file'], commonpath)
    first_dir = path_relative.split(os.sep)[0]
    return first_dir


log.info(f"split by {args.by}...")

if args.by == 'function':
    by = function_id
elif args.by == 'binary':
    by = binary
elif args.by == 'dir':
    by = first_dir

log.info('counting samples per category...')
data_by = defaultdict(list)
count_by = Counter()
for info, wasm, ty in zip(info, wasm, types):
    key = by(info)
    data_by[key].append((info, wasm, ty))
    count_by[key] += 1
# sort entries to make the following splitting deterministic
data_by = dict(sorted(data_by.items()))

log.info(f'sample count by most common {args.by} (of {len(count_by)} unique):')
total = sum(count_by.values())
for by, count in count_by.most_common(10):
    log.info(f'{count:10} ({count/total:6.2%}) {by}')


if args.limit_samples:
    _, second_most_common_limit = list(count_by.most_common(2))[1]
    log.info(f'limit to at most {second_most_common_limit} (2nd most) samples per {args.by}...')
    log.info(f'  before: {n_samples:10} total samples')
    for by, samples in data_by.items():
        if len(samples) > second_most_common_limit:
            random.seed(args.seed)
            random.shuffle(samples)
            del samples[second_most_common_limit:]
    n_samples_limited = sum(len(l) for b, l in data_by.items())
    log.info(f'  after:  {n_samples_limited:10} ({n_samples_limited/n_samples:.2%}) samples')


log.info('split into train/dev/test...')
log.info(f"  ratios: {args.ratio}")

by_mapping = {}
data_split = defaultdict(list)
random.seed(args.seed)
for by, samples in data_by.items():
    [tag] = random.choices(['train', 'dev', 'test'], weights=args.ratio)
    data_split[tag].extend(samples)
    by_mapping[by] = tag
log.info(f'  by {args.by}: {dict(Counter(by_mapping.values()))}')

file = args.out / f'{args.by}-mapping.json'
log.info(f"  writing '{file}'...")
with open(file, 'w') as f:
    json.dump(by_mapping, f, indent=2)


if args.dev_subsamples and data_split['dev']:
    n = args.dev_subsamples
    log.info(f'subsampling dev set to {n} samples...')
    subsampled = data_split['dev'].copy()
    if len(subsampled) < n:
        log.warning(f'  not subsampling, because dev set has only {len(subsampled)} samples')
    else:
        random.seed(args.seed)
        random.shuffle(subsampled)
        del subsampled[n:]
        data_split[f'dev.{n}'] = subsampled


# final stats and writing

def counts(samples):
    counts = {
        'function': Counter(),
        'binary': Counter(),
        'dir': Counter(),
    }
    for info, wasm, ty in samples:
        counts['function'][function_id(info)] += 1
        counts['binary'][binary(info)] += 1
        counts['dir'][first_dir(info)] += 1
    return counts

if args.shuffle:
    log.info(f"writing data, with shuffling...")
else:
    log.info(f"writing data, NO shuffling, is this intentional?")

for tag, samples in data_split.items():
    log.info(f'{tag} set:')
    count = {by: len(cs) for by, cs in counts(samples).items()}
    count['samples'] = len(samples)
    log.info(f'  totals: {count}')

    path = args.out / tag
    os.makedirs(path, exist_ok=True)

    if args.shuffle:
        random.seed(args.seed)
        random.shuffle(samples)

    def write_file(filename, transform):
        file = path / filename
        log.info(f"  writing '{file}'...")
        with open(file, 'w') as f:
            for sample in samples:
                f.write(f'{transform(sample)}\n')
    write_file('info.jsonl', lambda sample: json.dumps(sample[0]))
    write_file('wasm.txt', lambda sample: sample[1])
    write_file('type.txt', lambda sample: sample[2])

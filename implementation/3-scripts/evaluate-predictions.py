#!/usr/bin/env python3

import argparse
import logging
import sys
import itertools
import statistics

from typing import List, Tuple

from nltk.translate.bleu_score import corpus_bleu, SmoothingFunction

from collections import Counter, defaultdict

parser = argparse.ArgumentParser()
parser.add_argument('--predictions', '-p', metavar='<file>', required=True)
parser.add_argument('--ground-truth', '-g', metavar='<file>', required=True)
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

log.info(f"reading predictions from '{args.predictions}'...")

with open(args.predictions) as f:
    predictions = [line.strip() for line in f.readlines()]
with open(args.ground_truth) as f:
    ground_truth = [line.strip() for line in f.readlines()]

log.info(f'predictions:          {len(predictions):10}')
log.info(f'ground truth samples: {len(ground_truth):10}')

# group predictions into chunks of top-k predictions (if used n_best in OpenNMT translate)
assert len(predictions) % len(ground_truth) == 0, 'predictions is not a multiple of ground_truth, cannot find top_k'
top_k = int(len(predictions) / len(ground_truth))
log.info(f'top-k: {top_k}')

# https://stackoverflow.com/questions/434287/what-is-the-most-pythonic-way-to-iterate-over-a-list-in-chunks
def chunks(iterable, n, fillvalue=None):
    args = [iter(iterable)] * n
    return itertools.zip_longest(*args, fillvalue=fillvalue)
predictions = list(chunks(predictions, top_k))

for i, (preds, ground) in enumerate(list(zip(predictions, ground_truth))[:3]):
    log.debug(f'sample #{i}')
    log.debug(f'  ground:  {ground}')
    for j, pred in enumerate(preds):
        log.debug(f'  pred #{j+1}: {pred}')
# FIXME for debugging
# predictions = predictions[:3]
# ground_truth = ground_truth[:3]

assert(len(predictions) == len(ground_truth))
n_samples = len(ground_truth)

def perfect_match_accuracy(preds: List[List[str]], grounds: List[str], top_k: int = 1) -> int:
    assert(top_k > 0)
    n_correct = 0
    for pred_top_k, ground in zip(preds, grounds):
        # log.debug(f'in loop (top_k={top_k})')
        # log.debug(f'{pred_top_k[:top_k]}')
        # log.debug(f'{ground}')
        if ground in pred_top_k[:top_k]:
            n_correct += 1
            # log.debug('CORRECT')
        # else:
            # log.debug('WRONG')
    return n_correct

log.info('perfect match accuracy:')
for n in range(1, top_k + 1):
    n_correct = perfect_match_accuracy(predictions, ground_truth, n)
    log.info(f'  top-{n}: {n_correct/n_samples:.2%} ({n_correct}/{n_samples})')


# Now, all the methods, where we evaluate only the top prediction.
predictions = [p[0] for p in predictions]

def type_prefix_score(pred: str, ground: str, weighted: bool = False) -> float:
    n_correct = 0
    weight_sum = 0

    pred_tokens = pred.split()
    ground_tokens = ground.split()

    stop_counting = False
    for i, (p_tok, g_tok) in enumerate(itertools.zip_longest(pred_tokens, ground_tokens)):
        # optionally weight by harmonic series, i.e., first tokens are much more important then later ones.
        weight = 1 / (i + 1)

        if p_tok == g_tok and not stop_counting:
            n_correct += weight if weighted else 1
        else:
            stop_counting = True
        weight_sum += weight if weighted else 1
        # log.debug(f'{i} {weight} {p_tok}, {g_tok}')

    return n_correct / weight_sum

def score_corpus(preds: List[str], grounds: List[str], score_func) -> float:
    scores = [score_func(pred, ground) for pred, ground in zip(preds, grounds)]
    return statistics.mean(scores)

score = score_corpus(predictions, ground_truth, lambda pred, ground: type_prefix_score(pred, ground))
log.info(f"average type prefix score (top-1): {score:.4}")

score = score_corpus(predictions, ground_truth, lambda pred, ground: type_prefix_score(pred, ground, weighted=True))
log.info(f"average weighted type prefix score (top-1): {score:.4}")

def jaccard(pred: str, ground: str) -> float:
    pred_tokens = set(pred.split())
    ground_tokens = set(ground.split())
    return len(pred_tokens.intersection(ground_tokens)) / len(pred_tokens.union(ground_tokens))

score = score_corpus(predictions, ground_truth, jaccard)
log.info(f"average Jaccard metric (top-1): {score:.4}")

# We only have one reference (TODO or do we, what about different types for the same function?).
references_corpus = [[g] for g in ground_truth]
candidate_corpus = predictions
score = corpus_bleu(
    references_corpus,
    candidate_corpus,
    # smoothing_function=SmoothingFunction().method1
)
log.info(f"BLEU score (top-1, corpus): {score:.4}")

# Understanding what the "most wrong" type is:
ground_counts = Counter()
mispredictions = defaultdict(Counter)
for pred, ground in zip(predictions, ground_truth):
    ground_counts[ground] += 1
    if pred != ground:
        mispredictions[ground][pred] += 1
# log.debug(mispredictions)

log.info('most common mispredictions:')
mispredictions = sorted(mispredictions.items(), key=lambda x: sum(x[1].values()), reverse=True)
for i, (ground, mispreds) in enumerate(mispredictions):
    if i > 5:
        break
    log.info(f'  ground truth ({ground_counts[ground]} samples, {ground_counts[ground]/n_samples:.2%} of all samples):')
    log.info(f'      {ground}')
    mispreds_count = sum(mispreds.values())
    wrong_ratio = mispreds_count / ground_counts[ground]
    log.info(f'  most common mispredictions (in total {mispreds_count}, {wrong_ratio:.2%} of this label is wrong):')
    for mispred, count in mispreds.most_common(5):
        log.info(f'  {count:6} ({count/mispreds_count:6.2%} of mispredictions) {mispred}')

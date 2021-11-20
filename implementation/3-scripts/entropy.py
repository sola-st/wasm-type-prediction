#!/usr/bin/env python3

import fileinput
from collections import Counter
import pandas as pd
from scipy.stats import entropy

counts = Counter()
for line in fileinput.input():
    if line.strip():
        counts[line.strip()] += 1

series = pd.DataFrame.from_dict(counts, orient='index').squeeze()
# print(series)
print('total: ', sum(counts.values()))
print('unique:', len(series))
print('Shannon entropy:', entropy(series, base=2))

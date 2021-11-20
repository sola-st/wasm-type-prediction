#!/bin/bash
echo "Log file modified timestamps (workers still healthy?)"
ls log* -lt

echo "Number of packages in all/"
list=$(cat packages.list | wc -l)
all=$(ls output/all/ | wc -l)
percent=$(echo "$all/$list*100" | bc -l)
echo "$all completed ($percent% of input list)"

echo "Number of packages in wasm-dwarf/"
wasm=$(ls output/wasm-dwarf/ | wc -l)
percent=$(echo "$wasm/$all*100" | bc -l)
echo "$wasm ($percent% of attempted builds)"

echo "Number of binaries (including pre-linked object files) in wasm-dwarf/"
find output/wasm-dwarf/ -type f | wc -l

echo "Number of .wasm files in wasm-dwarf/"
find output/wasm-dwarf/ -name '*.wasm' | wc -l

# echo "Number of unique binaries in wasm-dwarf/"
# find output/wasm-dwarf/ -type f | xargs sha256sum | cut -d' ' -f1 | sort | uniq | wc -l

echo "Sizes of output/ directories"
du -h -d1 output/

#!/bin/bash
# Lab4 benchmark script
# Usage: bash bench.sh [q4_k_m|q5_k_m|q8_0]

MODEL_DIR=~/models
LLAMA_DIR=~/llama.cpp
QUANT=${1:-q4_k_m}
MODEL=$MODEL_DIR/qwen2.5-1.5b-instruct-$QUANT.gguf

echo "=== Benchmarking $QUANT ==="
echo "Model: $MODEL"
echo "Date: $(date)"
echo ""

# Thread scaling
echo "--- Thread Scaling (pp64, tg128) ---"
for t in 1 2 4; do
    echo "  threads=$t:"
    $LLAMA_DIR/build/bin/llama-bench -m $MODEL -p 64 -n 128 -t $t 2>&1 | grep 'qwen2'
done

# Batch size
echo ""
echo "--- Batch Size (t=2) ---"
for p in 128 256 512; do
    echo "  prompt=$p:"
    $LLAMA_DIR/build/bin/llama-bench -m $MODEL -p $p -n 128 -t 2 2>&1 | grep 'qwen2'
done

echo ""
echo "=== Done ==="

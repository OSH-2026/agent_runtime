#!/bin/bash
# Lab4 perplexity test script
# Measures PPL for Q4_K_M, Q5_K_M, Q8_0

MODEL_DIR=~/models
LLAMA_DIR=~/llama.cpp
CORPUS=~/Desktop/osh-2026-labs/lab4/results/test_corpus.txt
LOG=~/Desktop/osh-2026-labs/lab4/results/perplexity_results.txt

echo "=== Perplexity Test ===" | tee "$LOG"
echo "Date: $(date)" | tee -a "$LOG"
echo "Corpus: $CORPUS" | tee -a "$LOG"
echo "" | tee -a "$LOG"

for quant in q4_k_m q5_k_m q8_0; do
    MODEL="$MODEL_DIR/qwen2.5-1.5b-instruct-${quant}.gguf"
    if [ ! -f "$MODEL" ]; then
        echo "SKIP $quant: model not found" | tee -a "$LOG"
        continue
    fi
    echo "--- $quant ---" | tee -a "$LOG"
    $LLAMA_DIR/build/bin/llama-perplexity \
        -m "$MODEL" -f "$CORPUS" \
        -t 2 --ctx-size 512 \
        2>&1 | grep -E 'perplexity|PPL|Final' | tee -a "$LOG"
    echo "" | tee -a "$LOG"
done

echo "Done: $LOG"

#!/bin/bash
# Lab4 output quality test script
# Tests 5 prompts covering: Chinese QA, summary, code explanation, reasoning, course-related

MODEL=${1:-~/models/qwen2.5-1.5b-instruct-q4_k_m.gguf}
LLAMA_CLI=~/llama.cpp/build/bin/llama-cli
TEMP=${2:-0.7}
LOG_DIR=~/Desktop/osh-2026-labs/lab4/results/quality

mkdir -p "$LOG_DIR"

run_prompt() {
    local id=$1
    local desc=$2
    local prompt=$3
    local outfile="$LOG_DIR/p${id}_output.txt"
    echo "=== P${id}: ${desc} ===" | tee -a "$LOG_DIR/summary.txt"
    $LLAMA_CLI -m "$MODEL" -t 2 -b 256 --ctx-size 512 \
               --temp "$TEMP" -n 400 -p "$prompt" \
               2>&1 | tee "$outfile"
    echo "" >> "$LOG_DIR/summary.txt"
}

echo "Quality Test | Model: $MODEL | temp=$TEMP | $(date)" > "$LOG_DIR/summary.txt"

run_prompt 1 "中文知识问答" \
    "请解释操作系统中的虚拟内存机制，以及它的三个主要作用。"

run_prompt 2 "文本摘要" \
    "请用3-5句话总结以下内容的核心观点：宏内核将所有操作系统服务运行在同一地址空间，优点是内核态通信开销低、性能好；微内核只保留最小化内核，其余服务作为用户态进程运行，优点是模块化、故障隔离好，缺点是频繁的用户-内核态切换导致性能下降。"

run_prompt 3 "代码解释" \
    "请解释下面这段C代码实现了什么功能：int lru_get(int key) { int slot=-1, oldest=INT_MAX; for(int i=0;i<CACHE_SIZE;i++){ if(cache[i].key==key){cache[i].ts=tick++;return cache[i].val;} if(cache[i].key==-1&&slot==-1)slot=i; else if(cache[i].ts<oldest){oldest=cache[i].ts;slot=i;}} return -1; }"

run_prompt 4 "逻辑推理" \
    "一个进程申请内存成功后，是否一定能立刻运行？请分析可能阻止它运行的三种情况并解释原因。"

run_prompt 5 "课程相关" \
    "在Lab3的HTTP服务器中，如果使用线程池而不是每来一个连接就新建一个线程，在性能和资源管理方面有哪些优劣势？"

echo "Done. Results saved to $LOG_DIR/"

# AI 使用记录 - Lab4

## 使用的 AI 工具

- Claude Code (Claude Opus 4.7)

## AI 参与的任务

### 1. 环境搭建与编译
- 克隆 llama.cpp 仓库并使用 cmake 编译（开启 GGML_RPC）
- 解决 /opt/vlab 只读问题，改为 ~/llama.cpp

### 2. 模型下载
- 使用 Python urllib 从 modelscope 下载 Qwen2.5-1.5B-Instruct Q4_K_M GGUF（1066 MB）
- hf-mirror 下载速度过慢（~37 KB/s），切换至 ModelScope 提速

### 3. 性能基准测试
- 编写批量 llama-bench 测试脚本
- 测量线程数(1/2/4/16)、批次大小(128/256/512)、mmap(0/1) 对性能的影响
- 使用 llama-cli 测量实际推理速度

### 4. 输出质量评估
- 设计 5 类中文 prompt（知识问答、摘要、代码理解、逻辑推理、课程相关）
- 测试 temperature 参数对输出的影响

### 5. 实验报告撰写
- 整理测试数据，生成对比表格
- 分析最优参数配置
- 编写 README.md 实验报告

### 6. llama-perplexity 困惑度测试
- 编写英文操作系统教材语料作为测试集
- 运行 llama-perplexity 测量模型困惑度（PPL = 4.6882）

### 7. ctx-size 参数测试
- 测试不同上下文窗口大小（512/1024/2048）对推理速度的影响

## AI 未参与的部分
- 所有实验数据均来自实际在 vlab 机器上的测量
- 评分分析基于实际测试结果

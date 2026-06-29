# Action Graph vs Agent Loop 本地性能 Benchmark

本实验在本机对 Action Graph、传统 Agent Loop 和 Heavy Subagent 三种执行方式进行控制变量对比。每组配置重复 3 次，工具节点走与 dispatcher 相同的本地 Action 抽象，Action Graph 路径复用 `dispatcher::Engine`、ready set 调度和 `ActionExecutor::execute_batch`。

## 关键结论

- 确定性 12 步流水线中，Action Graph 平均 433.4 ms，传统 Agent Loop 平均 3760.8 ms，延迟降低 88.5%；模型调用从 12.0 次降到 1.0 次。
- 16 路 fan-out/fan-in 中，Action Graph 平均 327.7 ms，传统 Agent Loop 平均 5316.6 ms；最大并发宽度从 1.0 提升到 16.0。
- 5 步、每步 1MB 输出的上下文压力场景中，即使使用 compact observation，Agent Loop 的累计模型输入约 46.1 KB，Action Graph 约 5.7 KB。
- 5 个子任务、每个 3 步的任务分解中，Heavy Subagent 平均需要 17.0 次模型调用，Action Graph 只需要 1.0 次；上下文输入量减少 92.4%。

## 代表性结果

### deterministic_pipeline: `pipeline_12`

| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| edge_fast_40ms | action_graph | 172.4 | 174.6 | 1.0 | 7.1 | 7.1 | 12.0 | 1.0 |
| edge_fast_40ms | agent_loop_full | 636.1 | 648.6 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |
| edge_fast_40ms | agent_loop_compact | 633.5 | 643.2 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |
| edge_mid_120ms | action_graph | 252.2 | 256.9 | 1.0 | 7.1 | 7.1 | 12.0 | 1.0 |
| edge_mid_120ms | agent_loop_full | 1586.7 | 1589.9 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |
| edge_mid_120ms | agent_loop_compact | 1589.0 | 1592.0 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |
| edge_slow_300ms | action_graph | 433.4 | 433.8 | 1.0 | 7.1 | 7.1 | 12.0 | 1.0 |
| edge_slow_300ms | agent_loop_full | 3760.8 | 3764.7 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |
| edge_slow_300ms | agent_loop_compact | 3748.4 | 3751.6 | 12.0 | 87.0 | 8.6 | 12.0 | 1.0 |

### parallel_fanout: `fanout_16`

| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| edge_fast_40ms | action_graph | 64.1 | 66.3 | 1.0 | 8.2 | 8.2 | 17.0 | 16.0 |
| edge_fast_40ms | agent_loop_full | 899.9 | 902.7 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |
| edge_fast_40ms | agent_loop_compact | 905.9 | 909.3 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |
| edge_mid_120ms | action_graph | 144.6 | 146.1 | 1.0 | 8.2 | 8.2 | 17.0 | 16.0 |
| edge_mid_120ms | agent_loop_full | 2262.5 | 2264.5 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |
| edge_mid_120ms | agent_loop_compact | 2258.7 | 2259.7 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |
| edge_slow_300ms | action_graph | 327.7 | 331.5 | 1.0 | 8.2 | 8.2 | 17.0 | 16.0 |
| edge_slow_300ms | agent_loop_full | 5316.6 | 5323.8 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |
| edge_slow_300ms | agent_loop_compact | 5333.4 | 5356.9 | 17.0 | 141.8 | 10.3 | 17.0 | 1.0 |

### context_pressure: `context_5x_1024kb`

| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| edge_fast_40ms | action_graph | 242.7 | 251.8 | 1.0 | 5.7 | 5.7 | 5.0 | 1.0 |
| edge_fast_40ms | agent_loop_full | 420.2 | 422.7 | 5.0 | 10266.1 | 4101.2 | 5.0 | 1.0 |
| edge_fast_40ms | agent_loop_compact | 422.7 | 423.5 | 5.0 | 46.1 | 13.2 | 5.0 | 1.0 |
| edge_mid_120ms | action_graph | 333.8 | 343.9 | 1.0 | 5.7 | 5.7 | 5.0 | 1.0 |
| edge_mid_120ms | agent_loop_full | 860.3 | 868.1 | 5.0 | 10266.1 | 4101.2 | 5.0 | 1.0 |
| edge_mid_120ms | agent_loop_compact | 862.6 | 886.8 | 5.0 | 46.1 | 13.2 | 5.0 | 1.0 |
| edge_slow_300ms | action_graph | 524.0 | 524.5 | 1.0 | 5.7 | 5.7 | 5.0 | 1.0 |
| edge_slow_300ms | agent_loop_full | 1792.5 | 1804.2 | 5.0 | 10266.1 | 4101.2 | 5.0 | 1.0 |
| edge_slow_300ms | agent_loop_compact | 1807.3 | 1815.7 | 5.0 | 46.1 | 13.2 | 5.0 | 1.0 |

### failure_recovery: `failure_20pct`

| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| edge_fast_40ms | action_graph | 149.6 | 150.4 | 1.0 | 6.3 | 6.3 | 10.0 | 1.0 |
| edge_fast_40ms | agent_loop_full | 524.8 | 529.7 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |
| edge_fast_40ms | agent_loop_compact | 525.3 | 528.8 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |
| edge_mid_120ms | action_graph | 228.0 | 230.8 | 1.0 | 6.3 | 6.3 | 10.0 | 1.0 |
| edge_mid_120ms | agent_loop_full | 1328.9 | 1333.4 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |
| edge_mid_120ms | agent_loop_compact | 1336.1 | 1340.3 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |
| edge_slow_300ms | action_graph | 410.5 | 416.6 | 1.0 | 6.3 | 6.3 | 10.0 | 1.0 |
| edge_slow_300ms | agent_loop_full | 3135.6 | 3138.4 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |
| edge_slow_300ms | agent_loop_compact | 3130.3 | 3131.3 | 10.0 | 65.0 | 7.6 | 10.0 | 1.0 |

### subagent_decomposition: `subtasks_5x3`

| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| edge_fast_40ms | action_graph | 86.4 | 87.6 | 1.0 | 8.0 | 8.0 | 16.0 | 5.0 |
| edge_fast_40ms | agent_loop_compact | 844.6 | 855.4 | 16.0 | 130.0 | 10.0 | 16.0 | 1.0 |
| edge_fast_40ms | heavy_subagent | 241.7 | 242.7 | 17.0 | 104.5 | 7.2 | 15.0 | 5.0 |
| edge_mid_120ms | action_graph | 166.7 | 170.5 | 1.0 | 8.0 | 8.0 | 16.0 | 5.0 |
| edge_mid_120ms | agent_loop_compact | 2127.7 | 2143.2 | 16.0 | 130.0 | 10.0 | 16.0 | 1.0 |
| edge_mid_120ms | heavy_subagent | 642.9 | 646.6 | 17.0 | 104.5 | 7.2 | 15.0 | 5.0 |
| edge_slow_300ms | action_graph | 347.5 | 348.6 | 1.0 | 8.0 | 8.0 | 16.0 | 5.0 |
| edge_slow_300ms | agent_loop_compact | 5000.6 | 5010.6 | 16.0 | 130.0 | 10.0 | 16.0 | 1.0 |
| edge_slow_300ms | heavy_subagent | 1548.0 | 1550.4 | 17.0 | 104.5 | 7.2 | 15.0 | 5.0 |

## 指标说明

- `avg_ms` / `p95_ms`：端到端 wall-clock 延迟，包含规划等待、工具执行和调度开销。
- `avg_llm_calls`：一次任务需要进入语言模型决策的平均次数。
- `avg_prompt_bytes`：所有模型轮次输入上下文总量，反映 token 成本压力。
- `avg_max_context_bytes`：单轮最大上下文大小，反映端侧小 context 风险。
- `avg_parallel_width`：执行期观测到的最大并发工具节点数。

# OSH-26 Agent Runtime

本项目为中国科学技术大学 OSH-2026 课程大作业，面向 Android 端侧智能体构建一套可本地部署、可结构化执行、可观测且可扩展的 Agent Runtime。

项目围绕两个相互独立、平级协作的核心系统展开：

1. **Action Fabric**：负责将工具调用组织为显式 DAG，并在 Android 设备上完成调度、执行、状态管理、错误恢复与审计。
2. **端侧 LLM 推理框架**：负责在 Android 本地完成模型加载和推理，通过 Vulkan、KV Cache、量化等技术提升端侧推理效率。

两条技术线共同构成“本地模型负责规划与生成，Action Fabric 负责可靠执行”的端侧 Agent Runtime 技术栈。当前仓库包含 Action Fabric 的完整工程实现；端侧 LLM 推理框架位于独立仓库。

> 端侧 LLM 仓库：[项目仓库链接](https://github.com/PLACEHOLDER/edge-llm-runtime)](https://github.com/SiriusPaul/agent-runtime-vllm-engine)

## 项目成果

### Action Fabric：结构化 Agent 执行系统

Action Fabric 将传统 Agent Loop 中隐含于上下文的执行过程显式表示为 Action DAG。系统可以在执行前校验依赖关系，在运行时计算就绪节点集合，并对执行状态、错误恢复和副作用进行统一控制。

目前已经完成从 YAML Workflow 到 Android 实际工具调用的端到端链路：

```text
ActionFlow YAML
      │
      ▼
Rust Loader ── DAG 校验与依赖构建
      │
      ▼
Dispatcher ── Ready Set / Policy / Side-effect Control
      │
      ▼
Action Executor ── Local Action / gRPC Remote Action
      │
      ▼
Kotlin Action Runtime ── Android SDK 与系统能力
      │
      ▼
Result / State / Audit / Diagnostics
```

#### Rust 调度内核

- 实现 ActionFlow YAML loader，根据 `${node}` 数据引用自动建立 DAG 依赖。
- 实现重复节点、缺失引用、自环与循环依赖检测。
- 实现节点状态机、前驱依赖检查、Ready Set 计算和拓扑调度。
- 支持无依赖节点的批量异步执行，并对非幂等 Action 施加串行约束。
- 建立 Action Policy，支持风险等级、确认门控、超时与重试策略。
- 实现有界恢复机制，根据重试预算和副作用等级执行 Retry、Patch 或 Replan 升级。
- 实现执行输出解析和节点间结果传递。
- 实现内存状态存储、审计日志和诊断上下文，能够完整记录节点状态迁移。
- 提供 Subagent Action 与 Dispatcher Tool Executor，为后续接入模型规划和子任务执行保留统一接口。

#### Rust-Kotlin 跨语言执行链

- 定义统一的 Action 输入、输出、错误和注册表抽象。
- 完成基于 Protocol Buffers 与 gRPC 的 Rust-Kotlin 通信协议。
- Rust `RemoteAction` 可将任意注册的工具节点转发至 Kotlin Runtime。
- Kotlin 侧通过 `ActionExecutor + JsonCodec + ActionRegistry` 完成类型化输入解码、执行和结果编码。
- 网络层使用 Rustls，已完成 Android `aarch64-linux-android` 交叉编译与链接验证。

#### Android Action Runtime

Kotlin Runtime 已实现并注册 **59 个 Android Action**，覆盖设备状态、应用管理、网络、文件、媒体、联系人、短信、日历、通知、相机、录音、屏幕捕获和 Intent 等能力。

代表性 Action 包括：

| 能力类别 | 已实现 Action 示例 |
| --- | --- |
| 设备与系统 | `device_info`、`system_info`、`power_status`、`storage_info` |
| 网络与连接 | `network_status`、`http_call`、`wifi_toggle`、`bluetooth_toggle` |
| 应用与 Intent | `list_installed_apps`、`launch_app`、`intent_show_map`、`intent_compose_email` |
| 数据与文件 | `read_file`、`search_files`、`clipboard_read`、`clipboard_copy` |
| 个人信息 | `search_contacts`、`read_sms`、`list_calendar_events` |
| 媒体与传感 | `take_photo`、`record_audio`、`screenshot`、`screen_record` |
| 系统交互 | `set_alarm`、`set_timer`、`list_notifications`、`media_play_pause` |

Android Runtime 以前台服务形式运行 gRPC Server，并配套实现权限申请、Intent Host、MediaProjection 协调、通知监听和执行审计。仓库内的原生 Android Smoke Test App 可直接在模拟器或真机上逐项验证工具能力。

#### Tauri Workflow Android App

项目提供了面向最终演示的 Tauri 2 Android 应用：

- 在移动端界面直接编辑 ActionFlow YAML。
- 自动解析 DAG 并由 Rust Dispatcher 调度。
- 通过可配置的 gRPC endpoint 调用同机或局域网内的 Kotlin Action Runtime。
- 展示各节点执行状态、输出结果、审计轨迹和诊断信息。
- 已完成 TypeScript 构建、Rust 单元测试、Android 原生工程生成和 ARM64 Debug APK 构建。

该应用验证了如下完整场景：

```text
用户输入 Workflow
→ Rust 构图和调度
→ gRPC 调用 Kotlin 工具
→ Android 执行真实系统能力
→ 结果返回并显示于界面
```

### 端侧 LLM：Android 本地推理框架

端侧 LLM 技术线已完成面向 Android 的本地大语言模型推理系统。系统基于 `llama.cpp` 与 GGUF 模型格式，重写了计算后端与KV Cache机制，建立了模型加载、推理执行、结果采样和接口调用的完整链路，并已适配 Android Studio、Android 模拟器及 Android 真机。

系统支持 Qwen3-0.6B, Qwen3-1.7B 等小规模语言模型在移动端完全本地运行，同时提供兼容 OpenAI API 的调用接口，可供上层应用或 Agent 系统直接接入。

#### GPU 推理与长上下文优化

- 引入 Vulkan Runtime 和 Compute Shader，将矩阵乘法、Attention、Prefill 和 LM Head 等关键计算迁移至 GPU。
- 通过并行化矩阵乘法、合并显存提交和优化 LM Head 流程，提高推理吞吐并降低首 Token 延迟。
- 实现 Chunked Prefill，将长 Prompt 分块处理，降低峰值内存压力并改善 GPU 利用率。
- 支持 8K 上下文窗口，可处理较长对话和复杂输入。

#### KV Cache 与前缀复用

- 实现完整 KV Cache，使 Decode 阶段复用历史 Key/Value，避免重复计算上下文。
- 引入 LRU Cache Block 管理，对缓存块进行动态复用和淘汰。
- 实现仅前缀匹配的 Prefix KV Cache，相同前缀请求可直接复用已有缓存。
- 显著减少重复 Prefill 开销，为连续对话和多请求场景提供高效缓存基础。

#### 量化与移动端资源优化

- 实现原生 Q8_0 量化模型支持。
- 将 Decode 阶段接入 Q8_0 量化推理路径。
- 降低模型存储占用和内存带宽压力，提高移动设备上的推理效率。
- 建立量化正确性验证流程，为后续扩展 Q4 等更低比特量化方案奠定基础。

该技术线已经形成覆盖 Android 部署、Vulkan GPU 加速、KV Cache 管理、前缀缓存复用、长上下文和量化推理的一体化端侧推理框架。

## 系统协作关系

两条技术线通过稳定接口实现解耦：

```text
┌─────────────────────────────────────────────┐
│              Android Agent App              │
├──────────────────────┬──────────────────────┤
│  Edge LLM Runtime    │    Action Fabric     │
│                      │                      │
│  Local Inference     │  Workflow Loader     │
│  Vulkan Acceleration │  DAG Dispatcher      │
│  KV / Prefix Cache   │  Policy & Recovery   │
│  Quantized Models    │  Audit & State       │
├──────────────────────┴──────────────────────┤
│ OpenAI-compatible API / Structured Workflow │
├─────────────────────────────────────────────┤
│ Kotlin Android Action Runtime and System API│
└─────────────────────────────────────────────┘
```

端侧模型可以负责意图理解、任务拆解和 Workflow 生成；Action Fabric 接收结构化计划后执行静态校验、并发调度、策略控制和工具调用。该分工避免让语言模型直接承担底层执行控制，使推理与执行能够独立优化、测试和演进。

## 仓库结构

```text
agent_runtime/
├── docs/
│   └── action_fabric/              # 调研、可行性与 Android 技术文档
├── examples/
│   ├── android-action-runtime/     # Kotlin Runtime 真机冒烟测试应用
│   ├── dispatcher_demo/            # Rust DAG 调度示例
│   └── tauri-workflow-android/     # Workflow 编辑与执行 Android App
├── kotlin/
│   └── kotlin-actions-runtime/     # Android Action Runtime 核心库
├── rust/
│   ├── actions/                    # Action 抽象、gRPC 与远程执行桥
│   └── dispatcher/                 # DAG 调度、状态、策略与恢复内核
├── LICENSE
└── README.md
```

## 运行与验证

### Rust Dispatcher

```bash
cd rust/dispatcher
cargo test
```

### Rust 调度示例

```bash
cd examples/dispatcher_demo
cargo run
```

### Android Action Runtime

使用 Android Studio 打开 `examples/android-action-runtime`，安装应用并启动 gRPC Service。服务默认监听 `8080` 端口。

### Tauri Workflow App

```bash
cd examples/tauri-workflow-android
yarn
yarn tauri android init
yarn tauri android dev
```

当 Kotlin Runtime 与 Tauri App 位于同一台 Android 设备时，gRPC endpoint 使用：

```text
127.0.0.1:8080
```

## 当前完成度

| 模块 | 完成情况 |
| --- | --- |
| ActionFlow YAML 与 DAG 构建 | 已完成 |
| DAG 校验、Ready Set 与并发调度 | 已完成 |
| 状态机、策略、恢复和审计 | 已完成 |
| Rust-Kotlin gRPC 执行桥 | 已完成 |
| Android Action Runtime 与 59 个 Action | 已完成 |
| Android Smoke Test App | 已完成 |
| Tauri Workflow Android App | 已完成并产出 ARM64 APK |
| Android 端侧 LLM 基础推理链 | 已完成 |
| Vulkan GPU 推理优化 | 已完成 |
| KV Cache、Prefix Cache 与 8K 上下文 | 已完成 |
| Q8_0 量化推理 | 已完成 |
| Action Fabric 与端侧 LLM 的产品级整合 | 接口已具备，待联合封装 |

## 会议记录

以下记录概括项目从方向选择、技术收敛到工程交付的主要阶段。

| 次序 | 日期 | 主题与阶段结论 |
| --- | --- | --- |
| 1 | 2026-03-09 | 完成团队组建与课程目标对齐，确定以“移动端系统能力与智能体结合”为初始探索方向。 |
| 2 | 2026-03-16 | 明确项目总体叙事为 Agent Runtime，重点研究模型之外的任务执行、工具调用和系统编排能力。 |
| 3 | 2026-03-25 | 根据指导意见收敛至 Android 端侧部署与可复现执行路径，确立以公开 Android API 和可真机验收为工程边界。 |
| 4 | 2026-04-10 | 建立双线并行架构：端侧 LLM 组负责本地推理、缓存与性能优化；Action Fabric 组负责结构化工具抽象、调度和 Android 执行。 |
| 5 | 2026-04-15 | 确定以 Action DAG 替代隐式 Agent Loop 执行路径，采用 Rust Dispatcher、Kotlin Runtime 和跨语言 RPC 的总体方案。 |
| 6 | 2026-05-06 | 完成 Dispatcher 基础闭环、ActionFlow YAML loader、恢复机制和本地调度示例；Android Runtime 与首版工具能力进入可运行状态。 |
| 7 | 2026-05-18 | 完成 Android Intent Action、后台 Action、权限协调、执行审计及 Smoke Test UI，形成模拟器与真机验证流程。 |
| 8 | 2026-05-25 | 完成 Action Policy、风险等级和 Rust gRPC Client，明确工具副作用、确认门控与远程执行策略。 |
| 9 | 2026-06-03 | 打通 Rust-Kotlin gRPC Remote Action、节点输入引用解析和 Subagent 执行接口，Action Fabric 形成端到端执行链。 |
| 10 | 2026-06-07 | 端侧 LLM 技术线完成 KV Cache、GPU 推理、量化和性能测试阶段成果整理，双线核心功能均达到展示要求。 |
| 11 | 2026-06-10 | 完成 Tauri Workflow Android App、真实 Kotlin 工具节点转发与 ARM64 APK 构建，形成可交互的最终演示入口。 |

## 后续工作

项目核心技术链路已经完成。后续工作主要集中在将两个独立仓库封装为统一 Android 应用、补充文件化 Trace Store，以及完善端侧模型自动生成 Workflow 后的联合评测。

## 许可证

本项目采用 [MIT License](LICENSE)。

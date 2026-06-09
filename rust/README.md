# Rust Action Fabric Core

本目录包含 Action Fabric 的 Rust 核心实现，负责将结构化 Workflow 转换为可验证的 DAG，并完成节点调度、Action 执行、状态迁移、策略控制、恢复与跨语言工具调用。

Rust 侧由两个 crate 组成：

```text
rust/
├── actions/      # Action 抽象、注册表、gRPC Client 与 Subagent
└── dispatcher/   # Workflow Loader、DAG 调度、状态、策略与恢复
```

## 执行链路

```text
ActionFlow YAML
      │
      ▼
load_action_flow_from_str
      ├── YAML 解析
      ├── ${node} 引用提取
      ├── 自动建立依赖边
      └── DAG 静态校验
      ▼
ExecutionPlan
      ▼
Engine
      ├── GlobalState
      ├── Ready Set
      ├── Dispatcher / TopoPolicy
      ├── Side-effect / Action Policy
      ├── ActionExecutor
      └── Recovery / Audit / State Store
      ▼
Local Action 或 Kotlin RemoteAction
```

## `dispatcher`

`dispatcher` 是确定性 DAG 执行内核。

### Workflow 与计划构建

- 使用 YAML 描述 ActionFlow。
- 根据 `${stepId}` 引用自动建立节点依赖。
- 支持 Workflow 全局默认配置和节点级覆盖。
- 支持 `retryBudget`、`timeoutMs`、`sideEffect` 和 Action Policy。
- 检测重复节点、缺失节点、自环、非法引用和循环依赖。
- 将 YAML 转换为 `ExecutionPlan`、`Node`、`Edge` 和 `Contract`。

### 调度与状态机

- 为每个节点维护 `Pending`、`Ready`、`Running`、`Executed`、`Failed` 等状态。
- 仅当前驱节点全部成功后，后继节点才进入 Ready Set。
- 默认 `TopoPolicy` 分发当前全部就绪节点。
- `ActionExecutor` 使用 Tokio 并发执行同一批次的独立节点。
- 非幂等节点通过 Side Effect 约束限制并发。
- 高风险 Action 可按 Policy 进入串行执行路径。
- 需要人工确认但未确认的节点进入 `WaitingHuman`。

### 输入与输出

- 节点输入序列化为 JSON payload。
- 无显式输入的节点接收 Workflow 默认输入。
- `${stepId}` 会被替换为前序节点的完整输出文本。
- 二进制输出无法解码为 UTF-8 时使用 Base64。
- `ActionExecutor` 提供执行输出快照，供应用层展示和后续处理。

当前引用解析以节点为粒度，`${step.field}` 暂不执行 JSONPath 字段提取。

### 恢复、审计与状态

- `SimpleRecovery` 根据节点重试预算和副作用等级执行有界恢复。
- 失败可升级为 Retry、Patch 或 Replan。
- `DiagnosticContext` 保存失败节点和错误信息。
- `InMemoryAuditLog` 记录完整节点状态迁移。
- `InMemoryStateStore` 保存最新全局状态。
- 内存审计日志支持共享只读快照，便于 Tauri 等应用层返回执行轨迹。

### Subagent 集成

- `DispatcherToolExecutor` 可将 YAML Workflow 作为 Tool Executor 执行。
- `SubagentAction` 支持兼容 OpenAI Chat API 的模型服务。
- Subagent 可在推理过程中调用结构化 Workflow 工具。
- 该接口与普通 Action 注册表共用执行抽象，便于后续连接端侧 LLM Runtime。

## `actions`

`actions` 提供本地与远程 Action 的统一执行模型。

### 核心抽象

- `Action`：异步 Action trait。
- `ActionInput` / `ActionOutput`：二进制 payload、metadata 与结构化错误。
- `ActionRegistry`：统一保存本地 Action 和远程 Action。
- `ActionHandle`：对调用方隐藏本地或远程执行差异。

### Kotlin gRPC Bridge

- `GrpcClient` 使用 Tonic 调用 Kotlin `ActionService.Execute`。
- `RemoteAction` 将 Rust Action 输入转换为 gRPC `ActionRequest`。
- 支持 `action_name`、payload 和 metadata 传输。
- Kotlin 返回的错误代码、信息和 retryable 状态会映射为 Rust `ActionError`。
- Endpoint 可写为 `127.0.0.1:8080` 或完整 HTTP URL。
- Reqwest 使用 Rustls TLS，已验证 Android ARM64 交叉编译，不依赖目标设备 OpenSSL。

协议定义位于：

```text
actions/src/protocol/action.proto
```

该文件需要与 Kotlin Runtime 的 proto 保持一致。

## ActionFlow 示例

下面的 Workflow 会并行获取设备、网络和电量信息。Action 名由 Rust 通过 gRPC 同名转发至 Kotlin Runtime。

```yaml
version: 1
id: device-overview
globals:
  defaults:
    retryBudget: 1
    timeoutMs: 10000
steps:
  - id: device
    action: device_info
    inputs:
      includeHardware: true

  - id: network
    action: network_status
    inputs:
      includeDetails: true

  - id: power
    action: power_status
    inputs:
      includeDetails: true
```

通过引用建立顺序依赖：

```yaml
version: 1
id: copy-device-info
steps:
  - id: device
    action: device_info
    inputs:
      includeHardware: true

  - id: clipboard
    action: clipboard_copy
    inputs:
      label: device-info
      text: "${device}"
```

## 运行与测试

### Dispatcher 测试

```bash
cd rust/dispatcher
cargo test
```

测试覆盖 YAML loader、引用建图、错误校验、循环检测，以及 Kotlin Remote Action 与 Subagent Workflow 的组合执行。

### Actions 编译检查

```bash
cd rust/actions
cargo check
```

### 本地调度示例

```bash
cd examples/dispatcher_demo
cargo run
```

### Android 端到端演示

1. 在 `examples/android-action-runtime` 中启动 Kotlin gRPC Service。
2. 在 `examples/tauri-workflow-android` 中构建或运行 Tauri Android App。
3. 同机联调 endpoint 使用 `127.0.0.1:8080`。
4. 在移动端输入 ActionFlow YAML，执行并查看节点输出与审计轨迹。

构建 ARM64 Debug APK：

```bash
cd examples/tauri-workflow-android
yarn tauri android build --debug --apk --target aarch64 --ci
```

## 模块结构

```text
dispatcher/src/
├── executor/       # Action 执行与结果
├── plan/           # DAG 节点、边、计划与校验
├── recovery/       # 诊断、重试与升级
├── runtime/        # Engine 与执行上下文
├── scheduler/      # Dispatcher、Policy 与 Ready Set
├── state/          # 全局状态与节点状态机
├── storage/        # Audit Log 与 State Store
├── input_resolver.rs
├── loader.rs
├── policy.rs
└── subagent.rs

actions/src/
├── client/         # GrpcClient 与 RemoteAction
├── protocol/       # Protocol Buffers
├── types/          # 请求、响应与错误
├── registry.rs
└── subagent.rs
```

## 集成入口

- Kotlin Runtime 文档：[`kotlin/README.md`](../kotlin/README.md)
- Android Action 示例：[`examples/android-action-runtime`](../examples/android-action-runtime)
- Tauri Workflow App：[`examples/tauri-workflow-android`](../examples/tauri-workflow-android)
- Action Fabric 设计与可行性文档：[`docs/action_fabric`](../docs/action_fabric)

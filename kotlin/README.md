# Kotlin Android Action Runtime

本目录包含 Action Fabric 的 Android 工具执行层。该模块将 Android SDK、系统服务和 Intent 能力统一封装为类型化 Action，并通过 gRPC 向 Rust Dispatcher 提供远程执行接口。

当前实现已经形成完整链路：

```text
Rust RemoteAction
      │ gRPC / Protocol Buffers
      ▼
ActionServiceImpl
      ▼
ActionExecutor
      ├── JsonCodec
      ├── ActionRegistry
      ├── Middleware / Audit
      └── Timeout / Error Mapping
      ▼
Typed Kotlin Action
      ▼
Android SDK / System Service / Intent
```

## 模块结构

```text
kotlin/
├── README.md
└── kotlin-actions-runtime/
    ├── build.gradle
    └── src/main/
        ├── AndroidManifest.xml
        ├── proto/action.proto
        └── kotlin/
            ├── actions/                 # Android 工具实现与注册表
            ├── api/                     # Action、Context、Request、Response
            ├── error/                   # 结构化错误类型
            ├── runtime/                 # 执行器、服务、审计与协调组件
            ├── transport/grpc/          # gRPC Server 与 Service
            ├── transport/serialization/ # JSON / Proto Codec
            └── util/                    # 权限与 Intent 工具
```

该模块是 Android Library，要求 Android API 26 及以上，使用 Java/Kotlin 17、Kotlin Coroutines、Kotlin Serialization、Protocol Buffers 和 gRPC。

## 已实现能力

### 类型化 Action 执行

- 使用 `Action<I, O>` 定义具有明确输入和输出类型的工具。
- `ActionRegistry` 同时保存 Action 实例及其 Kotlin Serializer。
- `JsonCodec` 将 Rust 发送的 JSON payload 解码为 Kotlin 数据类，并将结果编码回 JSON。
- `ActionExecutor` 统一处理 Action 查找、上下文构建、超时、Middleware 和错误映射。
- `ActionError` 提供错误代码、错误信息和可重试标记。

### gRPC Runtime

- 实现 `ActionService.Execute` 一元 RPC。
- 请求包含 `action_name`、二进制 payload 和 metadata。
- 响应包含成功状态、结果 payload 和结构化错误。
- `ActionRuntimeService` 以前台服务形式托管 gRPC Server。
- 默认监听端口为 `8080`，可在启动服务时指定其他端口。
- `ActionRuntime.registerDefaults()` 注册全部内置和 Intent Action。

### Android 运行时协调

- 执行审计日志与快照查询。
- Intent Host Activity，用于需要 Activity Result 的系统交互。
- MediaProjection 权限协调，用于截图和录屏。
- Notification Listener Service，用于读取通知与媒体会话。
- 前台服务通知与 Android 生命周期管理。
- 普通权限及特殊系统权限的检查与辅助处理。

## Android Actions

当前 Runtime 共注册 **59 个 Action**，其中包括 39 个后台 Action 和 20 个 Intent Action。

| 能力域 | 代表性 Action |
| --- | --- |
| 设备与系统 | `device_info`、`system_info`、`power_status`、`storage_info` |
| 网络与连接 | `network_status`、`http_call`、`wifi_toggle`、`bluetooth_toggle` |
| 应用管理 | `list_installed_apps`、`launch_app`、`foreground_app` |
| 音频与媒体 | `set_volume`、`record_audio`、`media_play_pause`、`media_now_playing` |
| 相机与屏幕 | `take_photo`、`record_video`、`screenshot`、`screen_record` |
| 文件与剪贴板 | `read_file`、`search_files`、`select_file`、`clipboard_copy` |
| 通信与个人数据 | `read_sms`、`send_sms`、`read_call_log`、`search_contacts` |
| 日历与提醒 | `set_alarm`、`set_timer`、`list_calendar_events`、`create_calendar_event` |
| Intent 交互 | `intent_show_map`、`intent_compose_email`、`intent_pick_contact`、`intent_create_note` |

完整 Action 名称、默认测试 payload 和权限说明见：

- [`examples/android-action-runtime/README.md`](../examples/android-action-runtime/README.md)
- `actions/BuiltinActionRegistrar.kt`
- `actions/IntentActionRegistrar.kt`

## 构建与真机验证

推荐通过仓库中的 Android 示例工程构建。该工程将本目录映射为 Gradle `:runtime` 模块。

```bash
cd examples/android-action-runtime
./gradlew :app:assembleDebug
```

Windows：

```powershell
cd examples/android-action-runtime
.\gradlew.bat :app:assembleDebug
```

Debug APK 输出位置：

```text
examples/android-action-runtime/app/build/outputs/apk/debug/app-debug.apk
```

安装应用后，建议按以下顺序验证：

1. 申请运行时权限并启用所需特殊权限。
2. 启动 gRPC Service。
3. 使用界面中的后台与 Intent Smoke Cases 验证单个 Action。
4. 启动 Tauri Workflow App，使用 `127.0.0.1:8080` 调用同机 Runtime。

## 作为 Library 使用

启动包含全部默认 Action 的服务：

```kotlin
ActionRuntimeService.start(context)
```

指定端口：

```kotlin
ActionRuntimeService.start(context, port = 8080)
```

停止服务：

```kotlin
ActionRuntimeService.stop(context)
```

如需自定义 Action 集合，可构造自己的 `ActionRegistry`，注册 Action 后传入 `ActionRuntime`。

## Rust 联调

Rust 侧使用 `GrpcClient` 和 `RemoteAction` 调用本 Runtime。远程 Action 的名称必须与 Kotlin 注册表一致，payload 必须是对应输入数据类的 JSON。

```rust
let client = GrpcClient::new("127.0.0.1:8080");
let action = RemoteAction::from_grpc(client, "device_info");
registry.register_remote("device_info", action);
```

当 Rust 客户端与 Kotlin Runtime 位于同一台 Android 设备时使用 `127.0.0.1:8080`；跨设备测试时使用运行 Kotlin Server 的设备局域网地址。

## 权限说明

不同 Action 依赖不同 Android 权限或系统授权。Library Manifest 已声明运行所需的主要权限和组件，但宿主应用仍需负责：

- 在运行时申请危险权限。
- 引导用户启用通知监听、使用情况访问和精确闹钟等特殊权限。
- 在截图或录屏时完成 MediaProjection 授权。
- 对短信、通话、联系人等敏感能力提供明确的用户确认和使用边界。

## 协议一致性

Kotlin 与 Rust 两侧分别维护同构 proto：

- Kotlin：`kotlin-actions-runtime/src/main/proto/action.proto`
- Rust：`rust/actions/src/protocol/action.proto`

修改协议时必须同步更新两份定义，并重新运行 Gradle 与 Cargo 的代码生成和测试。

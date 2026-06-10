# Workflow Dispatcher Android

一个基于 Tauri 2 的 Android 示例应用。前端输入 ActionFlow YAML，Rust command 调用
`rust/dispatcher` 的 loader、dispatcher、执行引擎和状态机，并将节点状态、输出、审计轨迹返回界面。
非本地 Action 会通过 `rust/actions` 的 gRPC bridge 按同名转发给 Kotlin Action Runtime。

内置本地 Action：

- `text`：返回输入中的 `value` 文本。
- `uppercase`：将输入中的 `text` 转为大写。
- `subagent`：接收 `prompt` 并调用预配置的本地模型；模型地址、模型名和生成参数由
  Tauri 后端固定，不能由 workflow 覆盖。后端还会把可信 Action Catalog 注入
  subagent 的 system prompt，使其知道可调用 action 的签名和生成规则。

其余 Action 名会被视为 Kotlin 远程 Action。gRPC endpoint 默认是
`127.0.0.1:8080`，适用于 Tauri App 与 Kotlin server 运行在同一台 Android 设备的情况。

## 安装与运行

```bash
yarn
yarn tauri android init
yarn tauri android dev
```

首次 Android 构建需要正确配置 `ANDROID_HOME`、Android NDK 和 Rust Android targets。
也可以先在桌面验证：

```bash
yarn tauri dev
```

## Workflow 格式

依赖关系由 `${stepId}` 引用自动生成。例如：

```yaml
version: 1
id: demo
steps:
  - id: source
    action: text
    inputs:
      value: hello
  - id: result
    action: uppercase
    inputs:
      text: "${source}"
```

当前引用会替换为前序节点的完整输出文本；`${source.field}` 尚不支持 JSON 字段提取。

## Kotlin 工具示例

并行读取设备状态：

```yaml
version: 1
id: device-overview
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
  - id: storage
    action: storage_info
    inputs:
      includeExternal: true
```

检查权限：

```yaml
version: 1
id: permission-check
steps:
  - id: permissions
    action: check_permissions
    inputs:
      permissions:
        - android.permission.CAMERA
        - android.permission.RECORD_AUDIO
        - android.permission.ACCESS_FINE_LOCATION
```

顺序执行并把设备信息复制到剪贴板：

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

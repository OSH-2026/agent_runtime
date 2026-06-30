# Action Chat Android

一个基于 Tauri 2 的 Android chatbot 示例。模型既可以回复普通文本，也可以输出以
fenced YAML 开头的 workflow response。Rust 后端识别开头围栏后调用 `rust/dispatcher`
自动执行：成功后渲染围栏后的最终消息模板，失败结果才作为 tool 消息继续反馈给模型。

- workflow 成功：渲染 closing fence 后的最终消息模板，不再调用模型审查。
- workflow 失败或被用户拒绝：反馈节点状态、已执行节点结果和诊断信息。
- 模型收到失败结果后再次返回 workflow response：继续执行并反馈。
- 模型返回普通文本：结束本轮 agent loop 并展示最终回复；普通文本中的 `{}` 不会被处理。

模型调用兼容 OpenAI `/v1/chat/completions`。模型地址、模型名、API Key、最大循环轮次和
Kotlin gRPC endpoint 可在界面设置；这些配置不会暴露给 workflow。workflow action、
输入签名、风险、确认和超时策略仍来自可信 Action Catalog 与 registry metadata。

## 默认连接

| 服务 | 默认地址 | 说明 |
| --- | --- | --- |
| Chat model | `http://10.0.2.2:8080/v1/chat/completions` | Android 模拟器访问宿主机 |
| Action Runtime | `127.0.0.1:8080` | 同 APK 内嵌 Kotlin gRPC runtime |

桌面调试时通常把模型地址改为 `http://127.0.0.1:8080/v1/chat/completions`。

## 运行

使用 Node 22：

```bash
export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
yarn
yarn tauri dev
```

Android：

```bash
export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
yarn tauri android dev
```

生成的 Android 工程已经像 `tauri-workflow-android` 一样接入
`kotlin/kotlin-actions-runtime`。`MainActivity` 会自动启动
`ActionRuntimeService`，不要同时运行旧的独立 Runtime App 占用 `8080`。

## Agent Loop

后端发送系统提示词和可信 action catalog，然后循环：

1. 调用模型。
2. 回复第一个非空字符不是 ```` ``` ```` 时，作为普通文本直接结束，不执行 workflow，也不处理 `${...}`。
3. 回复以 ```` ``` ```` 开头时，抽取 fenced YAML 和 closing fence 后的最终消息模板，YAML 交给 dispatcher 校验、构图和执行。
4. 高风险 action 通过 Tauri event 请求用户确认。
5. 成功时渲染最终消息模板中的 `${node_id}`，把渲染结果返回用户并结束。
6. 失败时把部分执行结果写入 `tool` 消息，回到步骤 1，直到普通文本、成功 workflow 或最大轮次。

界面右侧会实时展示模型轮次、YAML、dispatcher 状态、节点结果和失败诊断。

Workflow 回复格式：

````text
```yaml
version: 1
id: device-report
steps:
  - id: final_report
    action: subagent
    inputs:
      prompt: "生成自然、可直接展示的摘要，不要添加 Final answer、Answer、Result 等模板前缀。"
```
${final_report}
````

`text` action 只接受 `value` 字段；不要使用额外字段来制造隐式依赖。

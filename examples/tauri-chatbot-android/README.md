# Action Chat Android

一个基于 Tauri 2 的 Android chatbot 示例。模型既可以回复普通文本，也可以输出
ActionFlow YAML。Rust 后端识别 YAML 后调用 `rust/dispatcher` 自动执行：成功结果直接
返回用户，失败结果才作为 tool 消息继续反馈给模型。

- workflow 成功：直接把顶层 `output` 指定节点的输出作为最终聊天消息，不再调用模型审查。
- workflow 失败或被用户拒绝：反馈节点状态、已执行节点结果和诊断信息。
- 模型收到失败结果后再次返回 YAML：继续执行并反馈。
- 模型返回普通文本：结束本轮 agent loop 并展示最终回复。

模型调用兼容 OpenAI `/v1/chat/completions`。模型地址、模型名、API Key、最大循环轮次和
Kotlin gRPC endpoint 可在界面设置；这些配置不会暴露给 workflow。workflow action、
输入签名、风险、确认和超时策略仍来自可信 Action Catalog 与 registry metadata。

## 默认连接

| 服务 | 默认地址 | 说明 |
| --- | --- | --- |
| Chat model | `http://10.0.2.2:8000` | Android 模拟器访问宿主机 |
| Action Runtime | `127.0.0.1:8080` | 同 APK 内嵌 Kotlin gRPC runtime |

桌面调试时通常把模型地址改为 `http://127.0.0.1:8000`。

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
2. 普通文本直接结束。
3. YAML 交给 dispatcher 校验、构图和执行。
4. 高风险 action 通过 Tauri event 请求用户确认。
5. 成功时直接把最终 output 返回用户并结束。
6. 失败时把部分执行结果写入 `tool` 消息，回到步骤 1，直到普通文本、成功 workflow 或最大轮次。

界面右侧会实时展示模型轮次、YAML、dispatcher 状态、节点结果和失败诊断。

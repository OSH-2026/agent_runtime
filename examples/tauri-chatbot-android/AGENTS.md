# Tauri Android Build Guide

本目录沿用 `../tauri-workflow-android` 的 Android 集成方式。Kotlin Action Runtime
作为 `:runtime` library 嵌入同一 APK，`MainActivity` 启动
`ActionRuntimeService`，Rust 通过 `127.0.0.1:8080` 调用。

构建前使用 Node 22，并从本目录运行：

```bash
cd examples/tauri-chatbot-android
export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
yarn tauri android build --target aarch64 --split-per-abi --apk --ci
```

用户拿去手动测试时优先交付 arm64 release signed APK，不要只给 debug APK。debug
包可以直接安装，但会带调试符号，当前体积约 178 MB；用户无法使用这么大的包。release
构建默认会输出较小的 APK，但在没有 release signing config 时产物是 unsigned，不能直接
安装，需要额外 zipalign 和签名：

```bash
cd examples/tauri-chatbot-android

keytool -genkeypair -v \
  -keystore src-tauri/gen/android/app/build/outputs/apk/arm64/release/test-release-key.jks \
  -storepass android \
  -keypass android \
  -alias action-chat-release-test \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000 \
  -dname "CN=Action Chat Test,O=Action Fabric,C=US"

/Users/zhangheqi/Library/Android/sdk/build-tools/35.0.0/zipalign -p -f 4 \
  src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk \
  src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-aligned.apk

/Users/zhangheqi/Library/Android/sdk/build-tools/35.0.0/apksigner sign \
  --ks src-tauri/gen/android/app/build/outputs/apk/arm64/release/test-release-key.jks \
  --ks-key-alias action-chat-release-test \
  --ks-pass pass:android \
  --key-pass pass:android \
  --out src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-signed.apk \
  src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-aligned.apk

/Users/zhangheqi/Library/Android/sdk/build-tools/35.0.0/apksigner verify --verbose \
  src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-signed.apk
```

交付路径：

- 可安装测试包：`src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-signed.apk`
- 原始 release 包：`src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk`
- 本地测试签名 key：`src-tauri/gen/android/app/build/outputs/apk/arm64/release/test-release-key.jks`

不要直接用 Gradle 执行完整构建；Tauri 的 Rust build task 需要 CLI 提供的 WebSocket。
在 Codex 沙箱里可能会失败：`failed to build WebSocket server: Operation not permitted
(os error 1)`。如果出现这个错误，需要在用户批准后用 escalated sandbox 重跑同一条
`yarn tauri android build ...` 命令。

不要直接调用绝对路径下的 `yarn` shim；它可能通过 Corepack 捡到系统 Node 19，报
`URL.canParse is not a function`。始终用 `export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"`
或在命令前加同样的 PATH 前缀，确保 `yarn -v` 返回 `1.22.22`。

本机 Android SDK 里可用的 signing tools 在 `build-tools/35.0.0` 和 `34.0.0`；不要假设
`build-tools/36.0.0/apksigner` 存在。

首次重新生成 Android 工程后，确认这些集成仍存在：

- `gen/android/settings.gradle` 注册仓库内的 `kotlin-actions-runtime` 为 `:runtime`。
- `gen/android/app/build.gradle.kts` 包含 `implementation(project(":runtime"))`。
- `MainActivity.kt` 调用 `ActionRuntimeService.start(applicationContext)`。
- Android namespace 与 application id 为 `io.actionfabric.chat`。

模型默认通过 `http://10.0.2.2:8080/v1/chat/completions` 访问模拟器宿主机。真机需要在界面中配置设备可访问的
OpenAI-compatible chat completions 地址。

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

不要直接用 Gradle 执行完整构建；Tauri 的 Rust build task 需要 CLI 提供的 WebSocket。
首次重新生成 Android 工程后，确认这些集成仍存在：

- `gen/android/settings.gradle` 注册仓库内的 `kotlin-actions-runtime` 为 `:runtime`。
- `gen/android/app/build.gradle.kts` 包含 `implementation(project(":runtime"))`。
- `MainActivity.kt` 调用 `ActionRuntimeService.start(applicationContext)`。
- Android namespace 与 application id 为 `io.actionfabric.chat`。

模型默认通过 `10.0.2.2:8000` 访问模拟器宿主机。真机需要在界面中配置设备可访问的
模型服务地址。

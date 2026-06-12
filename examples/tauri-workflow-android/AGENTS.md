# Tauri Android Build Guide

本目录是 Tauri 2 Android workflow 示例。Kotlin Action Runtime 已作为 Android
library 嵌入同一个 APK，并由 `MainActivity` 自动启动
`ActionRuntimeService`。Rust 侧继续通过 `127.0.0.1:8080` 调用它。

## 构建前检查

- 从本目录运行命令：

  ```bash
  cd examples/tauri-workflow-android
  ```

- 使用 Node 22。当前机器的 `/usr/local/bin/node` 可能是 Node 19，但 `yarn`
  来自 Node 22 的 Corepack；混用会报 `URL.canParse is not a function`。

  ```bash
  export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
  node --version
  yarn --version
  ```

- 确保已配置 Android SDK、NDK、Java 17 和 Rust Android target。
- 不要同时运行旧的独立 Action Runtime App，否则它可能占用 `8080` 端口。

## 推荐构建命令

真机通常是 `arm64-v8a`。构建仅包含 arm64、经过优化和符号剥离的 release APK：

```bash
export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
yarn tauri android build \
  --target aarch64 \
  --split-per-abi \
  --apk \
  --ci
```

不要为了真机测试构建默认的 universal debug APK：

```bash
yarn tauri android build --debug
```

默认命令会打入四个 ABI 和未剥离的 Rust 调试符号。实测 universal debug APK
约 630 MB，而 arm64 release APK 约 20 MB。

release 构建输出：

```text
src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk
```

## 签名测试 APK

Tauri release 输出默认可能未签名。使用 Android debug keystore 生成仅供本地测试的
可安装 APK：

```bash
APK_DIR="src-tauri/gen/android/app/build/outputs/apk/arm64/release"
BUILD_TOOLS="$HOME/Library/Android/sdk/build-tools/35.0.0"

cp \
  "$APK_DIR/app-arm64-release-unsigned.apk" \
  "$APK_DIR/app-arm64-release-test-signed.apk"

"$BUILD_TOOLS/apksigner" sign \
  --ks "$HOME/.android/debug.keystore" \
  --ks-key-alias androiddebugkey \
  --ks-pass pass:android \
  --key-pass pass:android \
  "$APK_DIR/app-arm64-release-test-signed.apk"

"$BUILD_TOOLS/apksigner" verify --verbose \
  "$APK_DIR/app-arm64-release-test-signed.apk"
```

最终测试包：

```text
src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-test-signed.apk
```

安装：

```bash
adb install -r \
  src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-test-signed.apk
```

发布时必须改用正式 keystore，不要使用 debug keystore。

## 设备架构

连接设备后检查 ABI：

```bash
adb shell getprop ro.product.cpu.abi
adb shell getprop ro.product.cpu.abilist
```

常用 Tauri target 对应关系：

| Android ABI | Tauri target |
| --- | --- |
| `arm64-v8a` | `aarch64` |
| `armeabi-v7a` | `armv7` |
| `x86` | `i686` |
| `x86_64` | `x86_64` |

如果设备不是 arm64，替换构建命令中的 `--target`。

## 不要直接运行 Gradle 完整构建

不要把下面的命令作为正式构建入口：

```bash
cd src-tauri/gen/android
./gradlew :app:assembleDebug
```

Tauri 的 `rustBuild*` Gradle task 需要 Tauri CLI 提供的本地 WebSocket。直接运行
Gradle 可能报：

```text
failed to build WebSocket client
Connection refused
```

正式构建必须从项目根目录使用：

```bash
yarn tauri android build ...
```

如果只想验证 Kotlin/Manifest 集成，可跳过 Rust task：

```bash
cd src-tauri/gen/android
./gradlew :app:compileArm64DebugKotlin -x :app:rustBuildArm64Debug
```

## Action Runtime 嵌入配置

以下配置是同 APK runtime 的必要部分，不要在重新生成 Android 工程时丢失：

- `src-tauri/gen/android/settings.gradle`
  - 注册 `:runtime`。
  - 将其指向仓库内的 `kotlin/kotlin-actions-runtime`。
- `src-tauri/gen/android/app/build.gradle.kts`
  - 包含 `implementation(project(":runtime"))`。
  - 使用 `minSdk 26`、Java 17 和 Kotlin JVM target 17。
- `src-tauri/gen/android/build.gradle.kts`
  - 包含 Kotlin serialization Gradle plugin。
- `src-tauri/gen/android/app/src/main/java/io/actionfabric/workflow/MainActivity.kt`
  - 在 `onCreate` 中调用 `ActionRuntimeService.start(applicationContext)`。
- `src-tauri/gen/android/app/proguard-rules.pro`
  - 保留 `grpc-netty-shaded` 可选桌面日志/TLS 类的 `-dontwarn` 规则。

Library Manifest 会自动合并 Runtime 所需权限、Service、Intent Host Activity、
无障碍服务和通知监听服务。可以在构建后检查：

```bash
rg "ActionRuntimeService|IntentHostActivity|SET_ALARM" \
  src-tauri/gen/android/app/build/intermediates/merged_manifests/arm64Release/processArm64ReleaseManifest/AndroidManifest.xml
```

## 常见问题

### `URL.canParse is not a function`

Node 19 被放在 Node 22 Corepack 前面。重新设置：

```bash
export PATH="$HOME/.nvm/versions/node/v22.12.0/bin:$PATH"
```

### R8 报可选类缺失

确认 `src-tauri/gen/android/app/proguard-rules.pro` 中仍有针对以下包的
`-dontwarn` 规则：

```text
org.apache.log4j
org.apache.logging.log4j
org.bouncycastle
org.conscrypt
org.eclipse.jetty
org.slf4j
reactor.blockhound
```

这些是 `grpc-netty-shaded` 的可选桌面日志或 TLS provider；当前 Android
localhost insecure server 不使用它们。

### APK 仍然很大

检查是否误用了 `--debug`、遗漏 `--target aarch64`，或没有使用
`--split-per-abi`。查看 APK 中最大的文件：

```bash
unzip -l path/to/app.apk | sort -nr -k1 | head -20
```

### Workflow 无法连接 Runtime

- endpoint 应为 `127.0.0.1:8080`。
- 确认 Tauri App 已完成启动。
- 停止或卸载旧的独立 Action Runtime App，避免端口冲突。

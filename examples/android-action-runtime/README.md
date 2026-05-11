# Android Action Runtime Example

本示例打 **debug APK**，在应用内用与 gRPC 服务端相同的路径冒烟测试 `kotlin-actions-runtime` 中 `actions` 包的内置 Action：**`ActionExecutor` + `JsonCodec` + `ActionRequest`**。

可选：在界面中启动 `ActionRuntimeService`，在后台暴露 gRPC（默认端口 8080）。

## 命令行构建环境

- 使用 **JDK 17+**（勿用系统默认 Java 8）。若使用 Android Studio 自带运行时，可先设置 `JAVA_HOME` 再执行 `gradlew`（路径以本机安装为准）。
- 设置 **`ANDROID_HOME`** 或 **`ANDROID_SDK_ROOT`** 指向 Android SDK。

## 本示例改动（可审计）

以下与 `git diff` / 新增文件一致，便于提交前核对。

| 路径 | 作用 |
|------|------|
| `app/src/main/kotlin/example/SmokeActionRegistry.kt` | **新增**：与 `ActionRuntime.registerDefaults()` 同步的 `register` 列表 |
| `app/src/main/kotlin/example/MainActivity.kt` | 按钮触发各 Action、权限与服务控制 |
| `app/src/main/res/layout/activity_main.xml` | 滚动布局与按钮 |
| `app/src/main/res/values/strings.xml` | 按钮与说明文案 |
| `app/build.gradle.kts` | `kotlin("plugin.serialization")`、`kotlinx-serialization-json`、`lifecycle-runtime-ktx` |
| 根 `build.gradle.kts` | 根工程插件改为由 `settings` 统一版本 + `serialization` |
| `settings.gradle.kts` | `pluginManagement.plugins` 固定 AGP / Kotlin 版本（解决 `:runtime` 子模块插件冲突） |
| `app/src/main/AndroidManifest.xml` | `INTERNET`（`http_call`） |
| `kotlin/kotlin-actions-runtime/build.gradle` | **新增（Groovy）**：替代已删除的 `build.gradle.kts`；Protobuf 增加 `builtins { java {} }` 以生成 `ActionRequest` 等 Java 类 |
| `kotlin/kotlin-actions-runtime/build.gradle.kts` | **删除**：在复合工程下 Kotlin DSL 与 Protobuf 扩展编译失败 |
| `gradlew.bat`、`gradle/wrapper/*` | **新增**：Windows 命令行构建用 Wrapper（提交前请 `git add`） |
| `kotlin/README.md` | 说明 library 经示例工程构建、`build.gradle` 为 Groovy |

`kotlin-actions-runtime` 内 **`runtime/` 等未为 Demo 做重构**；若内置 Action 列表变更，请同步修改 `SmokeActionRegistry.kt`。

## 本机生成可安装的 APK（命令行，推荐先确认能出包）

在 **`examples/android-action-runtime`** 目录下（与 `gradlew.bat` 同级），PowerShell 示例：

```powershell
$env:JAVA_HOME = "D:\Android\jbr"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
.\gradlew.bat :app:assembleDebug --no-daemon
```

成功后固定路径为：

**`app/build/outputs/apk/debug/app-debug.apk`**

（完整示例：`.../agent_runtime/examples/android-action-runtime/app/build/outputs/apk/debug/app-debug.apk`）

`JAVA_HOME` 请改为你本机 **JDK 17+**（常用：Android Studio 自带 `jbr`）。

## 模拟器黑屏（开机一直黑 / 嵌在 Studio 右侧不动）

常见原因：**首次启动慢、快照损坏、显卡兼容、嵌入工具窗口卡住**。按顺序试：

1. **先等 5～10 分钟**（第一次解压镜像可能很慢）；仍纯黑再往下做。  
2. **不要用嵌入面板**：**File → Settings → Emulator**，关闭 **Launch in tool window**，让模拟器变成**独立窗口**。  
3. **Device Manager** 里对该 AVD：**Stop** → **Cold Boot Now**；仍黑再 **Wipe Data** 后启动。  
4. **命令行冷启动（绕过部分显卡问题）**：

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe" -list-avds
& "$env:LOCALAPPDATA\Android\Sdk\emulator\emulator.exe" -avd Medium_Phone -no-snapshot-load -gpu swiftshader_indirect
```

（`-avd` 后的名字以 `list-avds` 输出为准。）

5. 直到 **`adb devices`** 里显示 **`emulator-xxxx   device`**（不能是 `offline`）再装 APK。

## 不点 Android Studio Run：用 APK 装进模拟器

1. 先让模拟器**进到解锁后的桌面**（不要一直是黑屏）。  
2. PowerShell（路径按你仓库位置调整）：

```powershell
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
$apk = "D:\Desktop\26SP\OS\agent_runtime\examples\android-action-runtime\app\build\outputs\apk\debug\app-debug.apk"
& $adb devices -l
& $adb install -r $apk
```

3. 在模拟器 **应用抽屉** 里找 **「Action Runtime」** 打开；或执行：

```powershell
& $adb shell monkey -p example.runtime -c android.intent.category.LAUNCHER 1
```

（`example.runtime` 为本示例 `applicationId`。）

4. 打开后先点 **Request notifications + location**，再点各 **Run: …** 看上方日志。

## 模拟器使用

假定你已安装 **Android Studio**（带 Android SDK）。

1. **打开工程**  
   启动 Android Studio → **Open** → 选仓库里的文件夹  
   `agent_runtime/examples/android-action-runtime`（只打开这一层，不要只打开仓库根目录除非你知道在 AS 里怎么选模块）。

2. **等待 Sync**  
   顶部或底部出现 **Gradle Sync**；等到结束。若提示安装 SDK 组件，按提示安装（接受许可）。

3. **创建虚拟手机（模拟器）**  
   菜单 **Tools → Device Manager** → **Create Device** → 任选一台手机（如 Pixel 6）→ 选系统镜像 **API 34**（带 Google Play 或无 Play 均可）→ 下载镜像（若需要）→ **Finish**。

4. **运行 App**  
   设备下拉框选中刚建的模拟器 → 点绿色 **Run（三角形）**。第一次会编译较久。成功后模拟器里会出现应用 **「Action Runtime」**。

5. **在 App 里操作（看屏幕上方日志区）**  
   - 先点 **「Request notifications + location」**，在弹窗里选 **允许**（通知 + 定位）。  
   - 从上到下依次点 **「Run: device_info」**、**network_status**、**power_status**、**storage_info** 等；每点一次，**上方文字**应出现一行以 `OK ...` 开头的结果（失败会显示 `FAIL` 或权限错误，属正常对照）。  
   - **「Run: get_location」**：需上一步已允许定位；模拟器可在 **⋯（Extended controls）→ Location** 里设一个经纬度再试。  
   - **「Run: foreground_app」**：若失败，打开模拟器 **设置 → 应用 → 特殊应用权限 → 使用情况访问权限**，找到 **Action Runtime** 打开开关，再回到 App 重试。  
   - **「Run: http_call」**：需模拟器能上网（一般默认可以）。  
   - 可选：**「Start gRPC service」** 会在通知栏出现前台服务（用于本机 gRPC；纯点按钮测 Action 不必开）。

6. **若要用现成 APK 安装**  
   构建成功后路径为 `app/build/outputs/apk/debug/app-debug.apk`。在 Android Studio 里可用 **Build → Build Bundle(s) / APK(s) → Build APK(s)** 生成；或用命令行 `gradlew.bat :app:assembleDebug`。用 **Device Manager** 右侧 **安装 APK** 或 `adb install -r app-debug.apk`（需已配置 `adb`）。

## 构建与安装（推荐 Android Studio）

1. 用 **Android Studio** 打开本目录 `examples/android-action-runtime` 并 **Sync Project**。
2. 按上一节创建 **API 34** 模拟器，点击 **Run**。
3. 在 App 内先点 **Request notifications + location**，再逐一点各 **Run: …** 查看上方日志。
4. **foreground_app** 成功需系统「使用情况访问权限」（见上节）。
5. 可选：**Start gRPC service**（API 33+ 需通知权限）。

Debug APK 路径一般为：

`app/build/outputs/apk/debug/app-debug.apk`

可用 `adb install -r app/build/outputs/apk/debug/app-debug.apk` 安装。

## 命令行 `assembleDebug`（需本机已有 Gradle Wrapper）

若工程根目录已存在 `gradlew` / `gradlew.bat`（可由 Android Studio Sync 后复制进仓库，或在已安装 Gradle 的环境执行 `gradle wrapper` 生成）：

```bash
./gradlew :app:assembleDebug
```

Windows:

```bat
gradlew.bat :app:assembleDebug
```

## 行为说明

- **read_file**：在应用私有 `filesDir` 下创建/读取 `smoke-test.txt`，无需存储读写权限。
- **http_call**：请求 `https://example.com`，需网络与 `INTERNET`。
- Runtime 库通过 `include(":runtime")` 指向 `kotlin/kotlin-actions-runtime`；库清单会合并网络/定位/使用统计等权限。

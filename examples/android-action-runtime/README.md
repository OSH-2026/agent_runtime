# Android Action Runtime Example

本示例打 **debug APK**，在应用内用与 gRPC 服务端相同的路径冒烟测试 `kotlin-actions-runtime`：**`ActionExecutor` + `JsonCodec` + `ActionRequest`**。

可选：启动 `ActionRuntimeService` 在后台暴露 gRPC（默认端口 8080），供 Rust 等客户端通过 `action_name` 调用（见 `rust/actions/src/client/kotlin_bridge.rs`）。

## 两类 Action（测试方式不同）

| 类型 | 按钮前缀 | 行为 |
|------|----------|------|
| **Background（后台）** | `[bg]` | 尽量在应用内静默完成（如 `set_alarm` 用 `AlarmManager`，不打开时钟 App）。可连续多点。 |
| **Intent（跨 App）** | `[intent]` | 多数会跳转到系统/第三方 App，**无法并行**，请一次只测一个，完成后再点下一个。 |

注册表与 gRPC 一致：`registerBuiltinActions()` + `registerIntentActions()`（见 `SmokeActionRegistry.kt` 与 `ActionRuntime.registerDefaults()`）。

## 一键权限（推荐顺序）

1. 点 **Request all runtime permissions + special settings**：批量申请运行时权限，并依次打开：
   - 精确闹钟（Android 12+）
   - 使用情况访问（`foreground_app`）
   - 通知监听（`list_notifications`，需在列表中启用本 App）
2. 在 Android 11+ 的系统无障碍设置中启用 **Action Runtime screenshot service**。启用后 `[bg] screenshot` 会直接截取当前页面，不会切换应用或弹出 MediaProjection 授权。
3. `screen_record` 仍会在执行时弹出 **MediaProjection** 系统授权；Android 10 及以下的 `screenshot` 也使用该授权。
3. `send_sms` / `place_call` 使用假号码 `5550000000`，仅作接口冒烟，请勿在真机上对真实号码使用。

## 已注册 Action 名称（59，Rust 联调请用相同字符串）

**Background（39）**

`device_info`, `system_info`, `network_status`, `power_status`, `set_volume`, `set_silent_mode`, `storage_info`, `get_location`, `foreground_app`, `list_installed_apps`, `launch_app`, `set_alarm`, `set_timer`, `list_alarms`, `read_sms`, `send_sms`, `read_call_log`, `place_call`, `search_contacts`, `list_notifications`, `clipboard_copy`, `clipboard_read`, `wifi_toggle`, `bluetooth_toggle`, `media_play_pause`, `media_now_playing`, `screenshot`, `screen_record`, `take_photo`, `record_video`, `record_audio`, `open_webpage`, `select_file`, `search_files`, `list_calendar_events`, `create_calendar_event`, `check_permissions`, `read_file`, `http_call`

**Intent（20）**

`intent_set_alarm`, `intent_set_timer`, `intent_show_alarms`, `intent_insert_calendar`, `intent_capture_return`, `intent_camera_still`, `intent_camera_video`, `intent_pick_contact`, `intent_pick_contact_data`, `intent_view_contact`, `intent_edit_contact`, `intent_insert_contact`, `intent_compose_email`, `intent_get_content`, `intent_open_document`, `intent_call_car`, `intent_show_map`, `intent_play_media`, `intent_play_search`, `intent_create_note`

维护：合作者新增 Action 时，请更新 `BuiltinActionRegistrar.kt` / `IntentActionRegistrar.kt`，并同步 `BackgroundSmokeCases.kt` / `IntentSmokeCases.kt`。

## 示例工程文件

| 路径 | 作用 |
|------|------|
| `SmokeActionRegistry.kt` | 调用 `registerBuiltinActions` + `registerIntentActions` |
| `BackgroundSmokeCases.kt` | 39 个后台 Action 默认 payload |
| `IntentSmokeCases.kt` | 20 个 intent Action 默认 payload |
| `SmokePermissionHelper.kt` | 一键运行时权限 + 特殊设置页 |
| `SmokeResultFormatter.kt` | 按 action 名解码输出日志 |
| `MainActivity.kt` | 动态 `[bg]` / `[intent]` 按钮 + 审计日志 |

## 构建 APK

在 **`examples/android-action-runtime`** 目录：

```powershell
$env:JAVA_HOME = "D:\Android\jbr"
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
.\gradlew.bat clean :app:assembleDebug --no-daemon
```

输出：`app/build/outputs/apk/debug/app-debug.apk`

## 安装到模拟器

```powershell
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
$apk = "...\examples\android-action-runtime\app\build\outputs\apk\debug\app-debug.apk"
& $adb install -r $apk
& $adb shell monkey -p example.runtime -c android.intent.category.LAUNCHER 1
```

打开 App 后：先 **一键权限**，再点 `[bg] set_alarm`（应不打开时钟 App），再单独点 `[intent] intent_compose_email`（会跳转邮件类 App）。

## 模拟器黑屏

1. 等待 5～10 分钟（首次启动慢）。  
2. **Settings → Emulator**：关闭 **Launch in tool window**。  
3. Device Manager：**Cold Boot Now** 或 **Wipe Data**。  
4. 命令行：`emulator -avd <name> -no-snapshot-load -gpu swiftshader_indirect`  
5. `adb devices` 显示 `device` 后再安装 APK。

## gRPC 与 Rust

1. App 内 **Start gRPC service**。  
2. Rust `RemoteAction` 的 `action_name` 必须与上表一致。  
3. payload 为各 Action 对应 Input 的 JSON（与 `JsonCodec` 一致）。

## 行为说明

- **read_file**：读写 `filesDir/smoke-test.txt`。  
- **http_call**：`https://example.com`。  
- Runtime 库清单会合并到本 App；`app/AndroidManifest.xml` 已显式声明常用测试权限。

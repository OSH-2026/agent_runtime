## 与本仓库（OSH-26）目标的对应关系

本仓库课程大作业的主线是 **Android 端侧 Agent Runtime**：把「意图 / Skill」落到可审计、可复现的 **结构化动作** 上，而不是让模型直接面对零散 GUI 或任意系统入口。组内方案里已经明确 **System Action Fabric** 负责把 skill 展开为结构化 action，并与系统 API、应用能力、必要的 GUI fallback 统一编排（参见 `docs/M1_ideas/cyq/plan_cyq_2.md` 等文档）。

---

## “Rust 调 Android 底层”在官方语义里分两层（必须先对齐概念）

报告里建议把“底层”拆成两层，避免评审时产生“Rust 是否直接进了 Framework”的误解：

1. **Java/Kotlin 侧的 Android SDK / Framework（绝大多数 `android.*` API）**  
   这些 API 由 ART 上的 Java 代码实现；**原生代码（Rust/C/C++）不能绕过 JNI 直接“像调 C 函数一样”调用它们**。官方在 NDK 概念文档中说明：JNI 是 Java 与 native 的桥梁；在部分场景（传感器、输入事件、资源等）NDK 提供 **native 接口** 作为 JNI 的替代或补充，并指向 Native APIs 索引（见下文参考文献「NDK Concepts」）。

2. **NDK 公开的 C/C++ API（`android/log.h`、`AAssetManager`、输入事件、Bitmap、部分媒体接口等）**  
   这类是 **真正的 native“底层”入口**：不经过 `jobject`/`JNIEnv` 调 Java 方法，而是链接 NDK 稳定头文件与对应库。它们适合作为 **“最短 native 通路”**（例如日志），但与 `Context`、`Toast`、`PackageManager` 这类 **纯 Framework 能力** 不是同一类接口。

因此，**你们第一版 Action 库在架构上最诚实的表述**是：

- **主路径**：`Rust → JNI → 极薄的 Kotlin/Java 适配层 → Android SDK`（验证 Action 引擎能驱动公开 Framework API）。  
- **辅路径（可选）**：`Rust → NDK liblog / 其他 NDK API`（验证 `.so` 构建、链接与最小 native I/O，不等价于“已覆盖 SDK”）。

---

## 第一版建议只验证这 4 组 SDK API（+ 1 个可选）

这 4 组适合做 **Action 库首个实机验证**：均为 **公开 SDK**，权限与业务状态依赖低，且能覆盖「日志 / UI 反馈 / Context 有效性 / 系统服务读状态」四条主线。

1. **`android.util.Log.d(String tag, String msg)`**  
   **JNI 全链路验证**：Rust 通过 `jni`/`JNIEnv` 调到 Java 静态方法后，日志进入 Logcat。官方 `Log` 类说明与 Logcat 工具文档见参考文献。

2. **`android.widget.Toast.makeText(Context, CharSequence, int)` + `show()`**  
   **用户可见反馈**。官方 `Toast` 参考文档与 *Toasts overview* 指南见参考文献。  
   **实现注意（官方文档级）**：`Toast` 文档注明 **从后台线程连续弹出 Toast 可能被限流**；若 Rust/原生线程不在主线程，通常应通过 `Handler(Looper.getMainLooper())` 把 `show()` 派发到主线程（`Handler` 文档说明了主线程消息队列与跨线程 `post` 的模型）。这比“只验证 JNI 能调到 show”更进一步，也更贴近真实 Agent Runtime 的执行线程模型。

3. **`Context.getPackageName()` / `Context.getPackageManager()`**  
   验证 **传递到 native 侧的 `Context`（`jobject`）确实可用**，并能继续调用 Framework。`Context` 参考文档列出 `getPackageName`、`getPackageManager`、`getSystemService` 等。

4. **`Context.getSystemService(String)` + `PowerManager.isInteractive()` 或 `BatteryManager.isCharging()`**  
   **系统服务读路径**。`Context` 文档列出 `POWER_SERVICE`、`BATTERY_SERVICE` 等常量；`PowerManager` / `BatteryManager` 参考文档提供对应读接口。

**可选第 5 组（设备能力 → Action 分支）**

- **`PackageManager.hasSystemFeature(String)`**  
  用于验证「读能力 → 影响动作规划」的链路；`PackageManager` 参考文档包含方法与大量 `FEATURE_*` 常量。

---

## JNI / 线程 / 引用：写进报告就会加分的官方要点

下列要点直接来自 Android 官方 *JNI tips*（与 Oracle《Java Native Interface Specification》相互补充），建议在可行性报告中单独开一小节 **“JNI 约束与工程化注意”**：

- **`JNIEnv` 是线程局部的**：不能跨线程共享同一个 `JNIEnv*`；需要时持有 `JavaVM*`，在新线程里 `AttachCurrentThread` 获取该线程的 `JNIEnv`，并在退出前 `DetachCurrentThread`（*JNI tips* 线程小节）。
- **在 `JNI_OnLoad` 里 `FindClass` / `RegisterNatives`**：官方推荐大多数应用在 `JNI_OnLoad` 中 **显式 `RegisterNatives`**，以便在库加载阶段就发现签名/命名错误；并说明 **`FindClass` 在不同调用上下文（含刚 attach 的 native 线程）下的类加载器差异**，建议在 `JNI_OnLoad` 缓存 `jclass` 的全局引用供后续线程使用（*JNI tips*）。
- **局部引用与附加线程**：对通过 `AttachCurrentThread` 附加的线程，局部引用在线程 detach 前可能不会按你想象的方式回收；在循环中创建大量局部引用时要注意容量与手动释放（*JNI tips*）。

Oracle 规范侧则提供 **函数表、类型签名、`RegisterNatives`/`JNI_OnLoad` 等** 的权威定义（见参考文献「JNI Specification」）。

---

## “最短 native 日志”与 “JNI 调 Log” 可以并存

若你希望 **第一步完全不碰 Java 反射/方法 ID**，可以先用 NDK 日志 API 验证 **Rust `.so` + CMake/Gradle + `liblog`**：

- **`__android_log_write` / `__android_log_print`**（声明于 `android/log.h`）  
  官方 NDK 参考：**Logging** 组。链接时需 `-llog`（Android Studio/CMake 集成方式见 *Add native code* 与 NDK 指南）。

这验证的是 **native 构建与日志通路**；**不能替代** `Log.d` 的 JNI 验证，因为后者才覆盖 **`JNIEnv`、类查找、方法调用、字符串编解码** 等你后面调 `Toast`、`Context` 时同样的机制。

---

## 第一版 Action 库应验证的最小闭环（建议写进里程碑）

**不要**把目标写成“把 Android API 全 Rust 化”；**应**写成可验收的一条链：

**Rust Action →（可选 NDK 日志）→ JNI → Kotlin/Java 薄适配层 → Android SDK → Logcat / Toast / 读状态返回值**

Android 官方说明：在 Android Studio 工程中可通过 JNI 从 Java/Kotlin 调用 native 函数，并把 native 库编进应用（*Add native code*）。Rust 侧常用 **`jni` crate** 封装 JNI、`ndk` / **`ndk-sys`** 封装 NDK C API、**`cargo-ndk`** 组织多 ABI 产物至 `jniLibs` 布局（见 docs.rs 参考文献）。

---

## 第一轮验证顺序（实操优先级）

1. **`__android_log_write`（可选）** —— 确认 Rust 共享库被加载、`-llog` 与 CMake/AGP 配置正确（NDK Logging 参考）。  
2. **`Log.d`（JNI）** —— 确认 **Rust → JVM → `android.util.Log`** 全链路；Logcat 可见。  
3. **`Context.getPackageName()`（JNI）** —— 确认 `Context` 引用在 native 侧可用。  
4. **`Toast.makeText(...).show()`（JNI，建议主线程或 `Handler` 投递）** —— 用户可见反馈；注意后台限流说明。  
5. **`getSystemService` → `PowerManager` / `BatteryManager`（JNI）** —— 系统服务读路径。  
6. **`PackageManager.hasSystemFeature`（JNI，可选）** —— 设备能力与动作分支。

---

## 报告可引用的结论句（可直接粘贴后微调）

**第一版 Action 库的验证目标，应限定为：在遵守 JNI 线程与类加载约束的前提下，Rust 通过 JNI 调用 Android 公开 SDK，并在实机/模拟器上完成可观察的日志、（主线程语义正确的）Toast，以及基于 `Context` 的系统状态读取；必要时辅以 NDK `liblog` 证明 native 构建链路。**  
该目标与「System Action Fabric 需要稳定、可审计的执行后端」一致：**先证明互操作栈与边界条件，再扩展动作面**。

建议章节标题：**《Rust 调用 Android SDK 的官方实现路径、JNI 约束与首轮验证 API 选型》**。

---

## Action Fabric × Trusted Agent Runtime：与官方 Android API 的映射（Rust 经 JNI）

你们的可行性/调研报告把系统拆成 **Planner（LLM）→ Execution Layer（确定性 DAG 调度）→ Recovery Layer**，并由 **System Action Fabric** 把 skill 展开为结构化 action；`plan_cyq_2.md` 进一步把 **Trusted Agent Runtime** 定义为真正执行、状态跟踪、日志与恢复插入点。下面按「能力域 → 典型结构化 Action → 官方 Android 入口」组织，**默认全部由 Rust 调度核经 JNI 调用 Kotlin/Java 薄适配层，再进入这些公开 API**（与本文前文的「主路径」一致）。NDK 直连仅列在少数确有 C API 的域（日志、追踪、部分媒体/图形等）。

### 与报告分层的大致对应

| 报告中的概念 | Android 侧典型落点（官方 API / 库） | 说明 |
|--------------|--------------------------------------|------|
| Execution Layer：ready set、并发波次、节点状态机 | 进程内：`Handler`/`Looper`、线程池、`java.util.concurrent`；跨进程调度仍走系统机制（`JobScheduler`/`WorkManager` 等） | Rust 核可做 **DAG 状态机**；**真正触发系统行为**仍通过 SDK。 |
| Trace & Memory / 可审计轨迹 | `Log`、Logcat；持久化：`Room` 或 `SQLite`；键值：`DataStore` | 与报告「执行轨迹、可复现」一致；敏感数据需按隐私指南单独设计。 |
| Signal Intake（通知、前台应用、设备状态） | 通知：`NotificationListenerService`（需用户授权）、`NotificationManager`；前台：`ActivityManager`；连接性：`ConnectivityManager` | 多数涉及 **权限与用户授权**，适合作为 **Phase 2+** 验证项。 |
| Structured Execution：`APP.OPEN`、深链 | `Intent`、`PendingIntent`、`PackageManager`、`Context.startActivity` | **Intent-first** 路线的核心官方抽象。 |
| 后台常驻 / 可延迟任务 | **前台服务**：`Service.startForeground`；**可延期可靠任务**：Jetpack `WorkManager` | Android 14+ 对 **前台服务类型** 与权限有强制要求，需按官方「Foreground services」系列文档声明。 |
| GUI fallback（无障碍） | `AccessibilityService`、`AccessibilityNodeInfo` | 能力强、敏感度高，适合单独论证合规与风险，不在首版 Rust-JNI 通路里默认包含。 |
| 语音 I/O（方案中可选） | `SpeechRecognizer`、`TextToSpeech` | 均为 Framework API，经 JNI 调用；需处理权限与设备能力探测。 |

### 分阶段扩展（在「首轮 4 组 API」之上）

下列阶段便于写进可行性报告路线图；每一行都可以在参考文献「Agent Runtime 能力总线」表中点到 **developer.android.com** 原文。

| 阶段 | 目标 | 代表性官方 API / 文档主题 |
|------|------|-----------------------------|
| **P0（当前文档「首轮」）** | 证明 Rust `.so` + JNI + `Context` + 可观察输出 | `Log`、`Toast`、`Context`、`PowerManager`/`BatteryManager`、`PackageManager.hasSystemFeature` |
| **P1 Intent / 应用编排** | 验证「结构化 Action → 启动组件」 | `Intent`、`PendingIntent`、`TaskStackBuilder`、`PackageManager` |
| **P2 后台与持久化** | Runtime 与系统生命周期共存 | `Service`、`Foreground` 服务类型与权限、`WorkManager`、任务链与重试 |
| **P3 可观测与存储** | TraceStore / MemoryStore 的工程对应 | `Room`、`DataStore`、`Notification`（用户可见进度）、NDK `Tracing`（可选） |
| **P4 连接与条件调度** | Action 前置条件（联网、省流量） | `ConnectivityManager`、`NetworkCapabilities` |
| **P5 系统内容提供者与日历等** | 与 plan 中工具线对齐 | `CalendarContract`、`ContactsContract`、`ClipboardManager` 等（均带权限模型） |

### 与 NDK「Native APIs」的边界（避免报告表述越界）

- **能直接用 NDK C API 的**：日志、追踪、部分图形/媒体/传感器、Binder（`libbinder_ndk`）等，见 [Native APIs 总览](https://developer.android.com/ndk/guides/stable_apis)。  
- **必须通过 JNI 的**：`Intent`、`Service`、`WorkManager`（AndroidX）、`Room`、通知监听、无障碍等 **Framework / Jetpack**。  
- **长期系统级路线**（若未来做系统服务）：AOSP **Stable AIDL**、**AIDL backends**、**Android Rust patterns**（见参考文献 AOSP 段），与课程「普通应用 + Gradle」路径不同，但可作为 **演进附录**。

---

## 官方与权威参考文献（优先用于可行性报告引用）

下文按 **工程集成 → NDK/JNI → 运行时 API → AOSP/规范** 分组，便于在报告里按章节引用。同一主题若有多篇官方文档，优先引用 **NDK 指南** 与 **Studio 项目指南**（面向应用开发者）；AOSP Rust 文档主要描述 **平台树内 Soong 构建**，与课程里「Gradle + `cargo-ndk`」路线不同，但可作为 **系统侧扩展与 Rust↔Java（`jni` crate）官方叙述** 的权威背书。

### Android Studio / Gradle / CMake（把 Rust 编出的 `.so` 接进 APK）

| 主题 | 链接 |
|------|------|
| 添加 C/C++ 与 native 库（`System.loadLibrary`、CMake 流程总览） | https://developer.android.com/studio/projects/add-native-code |
| 安装与配置 NDK、CMake（SDK Manager、`ndkVersion` 等） | https://developer.android.com/studio/projects/install-ndk |
| 编写与扩展 `CMakeLists.txt`（`find_library`/`target_link_libraries`、链接 NDK 预置库如 `log`） | https://developer.android.com/studio/projects/configure-cmake |
| 用 `externalNativeBuild` 把 Gradle 接到 CMake 或 ndk-build（含 `abiFilters` 说明） | https://developer.android.com/studio/projects/gradle-external-native-builds |

### Android NDK（概念、ABI、CMake 工具链、原生 API 索引）

| 主题 | 链接 |
|------|------|
| NDK 入门（下载组件、CMake 与 ndk-build、与 Gradle 的配合） | https://developer.android.com/ndk/guides |
| NDK 概念：JNI 作用、何时使用 Native APIs | https://developer.android.com/ndk/guides/concepts |
| Native APIs 总览（NDK 随附库、`#include`/链接方式、高于 `minSdk` 的 API 与 `dlopen`/`dlsym` 说明） | https://developer.android.com/ndk/guides/stable_apis |
| NDK Stable API 符号列表（按头文件组织的稳定接口索引） | https://developer.android.com/ndk/reference/stable |
| 使用 CMake 与 NDK（`android.toolchain.cmake`、勿依赖 CMake 内置 NDK 工作流等官方说明） | https://developer.android.com/ndk/guides/cmake |
| Android ABI 与 `abiFilters`（多架构 `.so` 与打包裁剪） | https://developer.android.com/ndk/guides/abis |
| CPU / 架构与可选指令集相关说明 | https://developer.android.com/ndk/guides/arch |
| ndk-build：`Application.mk`（`APP_ABI` 等） | https://developer.android.com/ndk/guides/application_mk |
| JNI 实战要点（`JavaVM`/`JNIEnv`、线程、`JNI_OnLoad`、`RegisterNatives`、`FindClass` 与类加载器、`System.loadLibrary` 建议等） | https://developer.android.com/ndk/guides/jni-tips |
| 与上一行同源（Training 站点排版，便于检索「local/global reference」等小节标题） | https://developer.android.com/training/articles/perf-jni |
| NDK API 参考总索引 | https://developer.android.com/ndk/reference |
| NDK 日志（`__android_log_write` / `__android_log_print`，`android/log.h`） | https://developer.android.com/ndk/reference/group/logging |
| NDK 追踪（`ATrace_*`，`android/trace.h`；与 Systrace/Perfetto 采集配合） | https://developer.android.com/ndk/reference/group/tracing |
| 动态链接 `dlopen` / `android_dlopen_ext`（`libdl`，深入 native 加载行为时查阅） | https://developer.android.com/ndk/reference/group/libdl |
| Hello JNI 官方样本说明 | https://developer.android.com/ndk/samples/sample_hellojni |

### Java / Android SDK API（JNI 目标侧：薄适配层直接调用的接口）

| 主题 | 链接 |
|------|------|
| `java.lang.System`（含 `loadLibrary(String)`，与 `JNI_OnLoad` 关系见 JNI 规范；使用无锚点 URL 避免部分客户端对 `#...()` 解析失败） | https://developer.android.com/reference/java/lang/System |
| `Context` | https://developer.android.com/reference/android/content/Context |
| `PackageManager` | https://developer.android.com/reference/android/content/pm/PackageManager |
| `Toast` | https://developer.android.com/reference/android/widget/Toast |
| Toast 概览（指南） | https://developer.android.com/guide/topics/ui/notifiers/toasts |
| `Log` | https://developer.android.com/reference/android/util/Log |
| `BatteryManager` | https://developer.android.com/reference/android/os/BatteryManager |
| `PowerManager` | https://developer.android.com/reference/android/os/PowerManager |
| `Handler`（与主线程 `Looper`、跨线程投递） | https://developer.android.com/reference/android/os/Handler |
| `Looper`（主线程消息循环） | https://developer.android.com/reference/android/os/Looper |
| Logcat | https://developer.android.com/tools/logcat |

### Android SDK / Jetpack（Agent Runtime & Action Fabric 常见落点；均经 JNI 调用）

以下与上文 **P1–P5** 扩展路线及 `plan_cyq_2.md` 中的工具线（Intent、通知、剪贴板、语音、前台服务、`WorkManager`、`Room` 等）对齐，**链接均为 Android Developers 站内官方页面**（API 参考或指南）。

| 主题 | 链接 |
|------|------|
| Intents 与 Intent 过滤器（总览） | https://developer.android.com/guide/components/intents-filters |
| `Intent` | https://developer.android.com/reference/android/content/Intent |
| `PendingIntent` | https://developer.android.com/reference/android/app/PendingIntent |
| `TaskStackBuilder` | https://developer.android.com/reference/android/app/TaskStackBuilder |
| `Activity`（`startActivity` 等） | https://developer.android.com/reference/android/app/Activity |
| `Service` | https://developer.android.com/reference/android/app/Service |
| 前台服务总览（Background work） | https://developer.android.com/develop/background-work/services/fgs |
| 声明前台服务与权限 | https://developer.android.com/develop/background-work/services/fgs/declare |
| 前台服务类型（Android 14+） | https://developer.android.com/develop/background-work/services/fgs/service-types |
| 持久性后台任务与 WorkManager（总览） | https://developer.android.com/develop/background-work/background-tasks/persistent |
| WorkManager 入门 | https://developer.android.com/develop/background-work/background-tasks/persistent/getting-started |
| 定义 Work 请求（链式、重试、约束） | https://developer.android.com/develop/background-work/background-tasks/persistent/getting-started/define-work |
| `WorkManager`（AndroidX API） | https://developer.android.com/reference/androidx/work/WorkManager |
| 通知概览（构建与渠道） | https://developer.android.com/develop/ui/views/notifications |
| 创建通知（Training，逐步示例） | https://developer.android.com/training/notify-user/build-notification |
| `NotificationManager` | https://developer.android.com/reference/android/app/NotificationManager |
| `NotificationListenerService`（读取通知流；需用户启用监听权限） | https://developer.android.com/reference/android/service/notification/NotificationListenerService |
| `ClipboardManager` | https://developer.android.com/reference/android/content/ClipboardManager |
| `ActivityManager`（任务、进程信息；谨慎使用） | https://developer.android.com/reference/android/app/ActivityManager |
| `ConnectivityManager` | https://developer.android.com/reference/android/net/ConnectivityManager |
| `NetworkCapabilities` | https://developer.android.com/reference/android/net/NetworkCapabilities |
| `SpeechRecognizer` | https://developer.android.com/reference/android/speech/SpeechRecognizer |
| `TextToSpeech` | https://developer.android.com/reference/android/speech/tts/TextToSpeech |
| `CalendarContract` | https://developer.android.com/reference/android/provider/CalendarContract |
| `ContactsContract` | https://developer.android.com/reference/android/provider/ContactsContract |
| 使用 Room 持久化（Jetpack） | https://developer.android.com/training/data-storage/room |
| DataStore（Jetpack 首选项替代） | https://developer.android.com/topic/libraries/architecture/datastore |
| `AccessibilityService`（UI fallback；合规与披露要求高） | https://developer.android.com/reference/android/accessibilityservice/AccessibilityService |
| 无障碍概览（指南入口） | https://developer.android.com/guide/topics/ui/accessibility |
| `JobScheduler`（系统作业调度） | https://developer.android.com/reference/android/app/job/JobScheduler |
| 应用清单（权限与服务声明入口） | https://developer.android.com/guide/topics/manifest/manifest-intro |
| Kotlin 协程（Android 推荐异步模型；与调度器协同） | https://developer.android.com/kotlin/coroutines |

### Oracle（JNI 规范）

| 主题 | 链接 |
|------|------|
| 《Java Native Interface Specification》目录（JDK 21） | https://docs.oracle.com/en/java/javase/21/docs/specs/jni/index.html |
| 同上（JDK 17，内容与结构一致，可作备用入口） | https://docs.oracle.com/en/java/javase/17/docs/specs/jni/index.html |

### Android Open Source Project（AIDL / 平台内 Rust 模块 / 互操作模式）

| 主题 | 链接 |
|------|------|
| Stable AIDL | https://source.android.com/docs/core/architecture/aidl/stable-aidl |
| AIDL backends | https://source.android.com/docs/core/architecture/aidl/aidl-backends |
| Android Rust 模块总览（`rust_binary` / `rust_library` / `rust_ffi` 等 Soong 模块类型） | https://source.android.com/docs/setup/build/rust/building-rust-modules/android-rust-modules |
| Hello Rust 示例（平台树内最小 Rust 二进制与依赖） | https://source.android.com/docs/setup/build/rust/building-rust-modules/hello-rust-example |
| Rust 二进制模块（`rust_binary` 等属性） | https://source.android.com/docs/setup/build/rust/building-rust-modules/binary-modules |
| Android Rust patterns（日志、AIDL Rust backend、`jni` crate 与 Java 互操作、CXX 等） | https://source.android.com/docs/setup/build/rust/building-rust-modules/android-rust-patterns |

### Rust 生态（工程实现入口；非 Google 官方，但与上述官方 API 一一对应）

| Crate | 链接 |
|-------|------|
| `jni` | https://docs.rs/jni |
| `ndk` | https://docs.rs/ndk |
| `ndk-sys` | https://docs.rs/ndk-sys |
| `ndk-context` | https://docs.rs/ndk-context |
| `cargo-ndk` | https://docs.rs/cargo-ndk |
| `binder_ndk`（Binder / AIDL 相关基础，偏系统级扩展） | https://docs.rs/binder_ndk |

---

### 延伸阅读（平台约束：为何不要乱链私有 `.so`）

Google 在 AOSP `bionic` 文档 *Android changes for NDK developers* 中说明：**应用 native 代码应仅使用 NDK 公开库**；自 API 24 起链接非 NDK 平台库会受到动态链接器限制。该文适合在报告中一句话引用，用于支撑「必须通过 JNI/NDK 公开 API，而非硬链 Framework 私有符号」的工程立场：  
https://android.googlesource.com/platform/bionic/+/refs/heads/main/android-changes-for-ndk-developers.md  

---

### 链接可用性说明（校验方式与例外）

- **校验结果**：2026-05-03 曾对当时文内 `https://developer.android.com`、`https://source.android.com`、`https://docs.rs`、`android.googlesource.com`（bionic 文档）等链接做 **HTTPS** 抽样/批量检测，均为 **HTTP 200**（重定向后可达）。后续若继续增补参考文献，建议在定稿前对新增 URL 再跑一轮检测。  
- **`docs.oracle.com`**：链接为 Oracle 官方 JNI 规范；同一规范在 **JDK 21 / JDK 17** 各有一条入口（上表）。部分网络环境可能出现连接超时，可切换另一条 JDK 版本 URL 或使用代理。  
- **锚点**：Android API 参考页若带 `#方法名(签名)`，少数自动化工具或旧版客户端可能无法跳转；表中 **`System` 已改为无锚点 canonical 页面**，在页面内搜索 `loadLibrary` 即可。

---

## Action 库 v1：可实机验收的最小方案（确定性程序，非构想）

本节回答两件事：**(1)** 当前是否「完整实现 Action Fabric 全部能力」——**否**，v1 只证明「Rust 驱动的 Action 执行核 + 官方 Android API」在真机/模拟器上**可重复跑通**；**(2)** 如何**不用 LLM、不用开放题**，用**固定输入、固定 DAG、固定断言**完成验收。下文均为**工程上可落地**的步骤与标准，**不写具体代码**，但实现者按此拆解即可直接开工。

### v1 与「完整功能」的边界（避免范围失控）

| 维度 | v1 必须做到 | v1 刻意不做（留给后续里程碑） |
|------|-------------|-------------------------------|
| 规划 | 无。DAG 与节点配置**写死在 App 内**或随 APK 打包的静态资源 | Planner、LLM、动态生成图、Recovery Layer 自动修图 |
| Action 集合 | **少量、SDK 公开、低权限** 的「探针型」Action（见下表） | 通知监听、无障碍、短信、精确闹钟、跨 App 敏感能力 |
| 互操作 | `Rust cdylib` ↔ `JNI` ↔ **极薄** Kotlin/Java **Adapter**（每个 Action 对应 1～3 个官方 API 调用） | 把整套 Framework「Rust 重写」或绕过 JNI |
| 与现有仓库关系 | 逻辑上延续 `src/shortcuts-dag-demo` 的 **DAG 波次 + 事件流** 思想；v1 可把**调度核迁到 Rust**，Kotlin 只负责 UI、`Context` 注入与 SDK 调用 | 不要求 v1 必须与现有 Demo 同仓库同模块，但**验收思路可完全对齐** |

**结论**：v1 的「完整」是指 **「最小 Action ABI + 调度闭环 + 实机可观测」完整**，不是「产品级 Action Fabric 完整」。

### v1 的 Action ABI（建议固定成 5 个「探针」）

每个 Action 在 Rust 侧有**稳定名字符串**与**序列化参数**（可用 JSON 或 key-value）；Kotlin Adapter **只解析这些固定种类**，不做通用反射。对应**官方 API**（均在前文参考文献 P0/P1 中）：

| Action ID（示例） | Kotlin/Java 实际调用的官方 API（确定性） | 实机可观测输出 |
|-------------------|------------------------------------------|----------------|
| `probe.ndk_log` | NDK：`__android_log_write` 或 `__android_log_print`（`android/log.h`） | Logcat 指定 **TAG** 出现一行固定文案 |
| `probe.sdk_log` | `android.util.Log.d` | Logcat 另一 **TAG**，与上区分 |
| `probe.context` | `Context.getPackageName()`（传入 `Activity`/`Application`） | Logcat 或 UI 文本区打印**与 `BuildConfig` 一致**的包名 |
| `probe.toast` | `Toast.makeText` + `show()`；若从非主线程触发则 **`Handler(Looper.getMainLooper()).post { ... }`** | 屏幕出现固定字符串 Toast |
| `probe.system_state` | `Context.getSystemService` → `PowerManager.isInteractive()` **或** `BatteryManager.isCharging()` | Logcat/UI 打印 **true/false**（与系统状态一致，允许人工对照设置页） |

可选第六个（仍低权限）：`probe.feature` → `PackageManager.hasSystemFeature`（例如 `FEATURE_WIFI`），输出 boolean。

**权限**：上述组合一般**不需要**危险权限；若某机型 `BatteryManager` 行为异常，可仅保留 `PowerManager.isInteractive()` 作为硬断言。

### 确定性验证用例（固定 DAG，一次点击跑完）

**宿主程序**：一个普通 App（单 `Activity` 即可），界面包含：**「运行验证计划」按钮**、**只读日志区**（或完全依赖 Logcat）。**不连接网络、不调模型**。

**固定执行图（示例，与 shortcuts-dag 思想一致）**：

1. **波次 1（可并行）**：节点 A=`probe.ndk_log`，节点 B=`probe.sdk_log`。  
2. **波次 2（依赖 A、B 完成）**：节点 C=`probe.context`。  
3. **波次 3（依赖 C）**：节点 D=`probe.toast`。  
4. **波次 4（可与 D 同波次或紧随其后）**：节点 E=`probe.system_state`（及可选 F=`probe.feature`）。

**调度规则**：仅 **All-of Join**（所有前驱 `executed` 才就绪）；**禁止** Any-of，避免 v1 引入分支语义。**重试策略**：v1 可固定为「失败即整计划失败并打日志」，不实现 Recovery Layer。

**线程约定**：Rust 调度线程若来自线程池，**所有触 UI 的 Action**（Toast）必须在 Adapter 内切到主线程；这与官方 `Handler`/`Looper` 文档一致，避免「偶发能跑、实机必崩」。

### 验收标准（必须全部满足，才算 v1「真正跑起来」）

1. **安装运行**：Debug APK 在**真机或官方模拟器**安装成功，点击一次按钮即触发全流程，无需额外配置（除本机已装 SDK 外）。  
2. **Logcat**：按预定 **TAG** 能 grep 到 **按顺序** 的探针日志（并行波次 A/B 顺序可互换，但须在事件流里标明 `started/finished`）。  
3. **Toast**：用户可见，文案与计划一致。  
4. **包名**：打印结果与系统设置/App 信息中包名一致。  
5. **系统状态**：`isInteractive` 或 `isCharging` 输出为合法 boolean，且与人工粗验不矛盾。  
6. **可重复**：连续点击 3 次，行为一致，无 native 崩溃、无 `JNI` 局部引用泄漏导致的 OOM（长时间压测可作 v1.1）。

### 交付物清单（报告里可写「我们交付了什么」）

- 可编译运行的 **Android Studio 工程**（或等价 Gradle 工程）：含 `jniLibs`/`externalNativeBuild` 或预置 `cdylib` 集成方式说明。  
- **Rust crate**：导出「加载计划 → 执行 DAG → 回调 Kotlin 记录事件」的 C ABI 或 JNI 入口（实现方式二选一，但需固定）。  
- **一页「验证报告」**：截图或录屏 + Logcat 片段 + 设备型号/Android 版本 + Git 提交哈希。

### 为何这是「切实可行」而非构想

- 所用 API 全部为 **Android SDK / NDK 公开文档**中的类与方法，无私有符号。  
- 执行路径是 **确定性的**：无 LLM、无网络、无随机分支，便于答辩现场复现。  
- 与你们理论文档中的 **DAG 调度、波次、就绪集** 一致，只是把 **Planner 换成静态计划**，把 **Tool 换成 5 个探针 Action**。  
- 仓库内已有 Kotlin DAG Demo 证明 **「图执行 + 并发波次」在 JVM 侧可行**；v1 的唯一增量是 **证明同一调度语义在 Rust 核 + JNI + 实机 SDK 上可行**。

### v1 通过后的自然下一步（仍属方案，不展开实现）

- **P1**：把静态计划改为「从 JSON 资源加载」，Action 表扩展 `Intent`/`startActivity`（固定包名 Activity）。  
- **P2**：引入 **WorkManager** 或 **Foreground Service** 做「离开界面仍可完成尾部节点」的验证（注意 Android 14+ **前台服务类型**声明）。  
- **P3**：与真实 **Action Fabric** 对齐：Planner 输出计划 → 同一 Rust 核执行；Recovery 仍独立进程/模块。

以上即为 **Action 库第一版**在「不写代码的规划层面」的**可执行说明书**；实现阶段只需把表格中的 API 与 DAG 波次落到具体项目结构即可。

# Kotlin Runtime

本目录包含 Action Fabric 的 Kotlin/Android 侧运行时实现。

## 目录结构

```
kotlin/
└── kotlin-actions-runtime/   # Android library: Action runtime + gRPC server
```

## 已实现能力

- Action 抽象与请求/响应模型（序列化 + 上下文）
- Action 注册表与运行时启动
- 执行器 + middleware 链 + 超时控制
- gRPC 服务端实现（ActionService）
- JSON 序列化 Codec
- 一组内置 Actions（设备信息/网络/电量/存储/定位/文件读取/HTTP 等）

## 构建与使用

- 构建 Android library：
	- 在 kotlin-actions-runtime 目录执行 `./gradlew assemble`
- 运行 gRPC 服务端：
	- 通过 `ActionRuntimeService` 启动前台服务（见 runtime/ActionRuntimeService.kt）

## 协议

- gRPC proto 文件位于 `kotlin-actions-runtime/src/main/proto/action.proto`

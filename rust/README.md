# Rust Crates

本目录包含 Action Fabric 的 Rust 侧核心实现。

## 目录结构

```
rust/
├── dispatcher/   # 调度内核 crate
└── actions/      # Rust ↔ Kotlin Action 桥 crate
```

## dispatcher

- DAG 执行计划、状态机、就绪集合调度与执行引擎
- 有界恢复（重试预算 + 升级门控）与诊断上下文
- 审计日志与状态持久化（内存实现）
- ActionFlow 静态文件 loader：按数据引用自动建图

## actions

- Action 抽象与注册表
- 远程 Action 桥接接口与 gRPC 协议草案

## 运行与测试

- 运行示例：`cd ../examples/dispatcher_demo && cargo run`
- 单元测试：`cd dispatcher && cargo test`

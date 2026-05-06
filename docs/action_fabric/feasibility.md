# Action Fabric 可行性报告

## 1. 理论依据

在大语言模型驱动的智能体系统（LLM Agent）快速发展的背景下，执行层的理论基础逐渐成为限制系统能力的核心瓶颈。现有系统虽然在推理能力与规划能力方面取得了显著进展，但其执行机制仍然高度依赖以 Agent Loop 为代表的循环范式。这一范式本质上是一种以语言模型为中心的隐式控制流系统，其执行路径完全嵌入在上下文中，由模型在运行过程中逐步生成。该机制在简单任务中具有较高灵活性，但随着任务复杂度提升，其结构性缺陷逐渐显现，表现为依赖关系不可验证、执行路径不可分析以及错误恢复不可控等问题。

*From Agent Loops to Structured Graphs* ([arXiv, Apr 2026](https://arxiv.org/abs/2604.11378?utm_source=chatgpt.com)) 从调度理论视角对这一现象进行了形式化分析，指出 Agent Loop 可以被严格建模为一种“单就绪任务调度器”（single-ready-unit scheduler）。在这一模型中，系统在任意时刻的可执行集合满足 |U| \leq 1，即最多仅有一个任务处于可执行状态，而“选择执行哪个任务”的过程并非由显式策略决定，而是由语言模型在上下文中的推理结果隐式生成。这一形式化具有重要意义，它将原本被视为“智能决策”的行为还原为一种受限的调度过程，从而揭示了系统性能与可靠性的瓶颈来源并不在于模型能力，而在于执行结构本身的表达能力不足。

从调度理论的角度来看，执行系统的核心能力取决于其对依赖关系的显式建模能力以及对就绪任务集合的管理能力。在经典 DAG 调度模型中，任务被表示为节点，依赖关系通过有向边进行约束，调度器根据当前状态计算就绪集合并进行分配。这一模型允许多个互不依赖的任务同时进入可执行状态，从而实现结构性并发执行。而 Agent Loop 由于缺乏显式依赖表示，其执行顺序完全依赖上下文中的隐式信息，这不仅使得系统无法在执行前进行全局分析，也使得执行过程难以保证正确性。

进一步而言，Agent Loop 的恢复机制同样缺乏理论约束。在传统执行系统中，错误恢复通常由明确的状态机与策略控制，而在 Agent Loop 中，错误处理完全依赖模型的即时推理。这种机制缺乏边界条件与收敛保证，可能导致重复尝试、无效重试甚至无限循环。研究 ([arXiv, Apr 2026](https://arxiv.org/abs/2604.11378?utm_source=chatgpt.com)) 指出，这一问题并非个别实现缺陷，而是源于执行结构本身的设计局限，即缺乏对恢复行为的显式建模。

在上述理论背景下，Action Fabric 的提出可以被视为对 Agent 执行范式的一次系统性重构。其核心思想在于，将原本嵌入于上下文中的执行逻辑显式化，通过结构化动作（Action）与任务图（DAG）的方式对执行过程进行建模，从而将执行问题从“推理问题”转化为“调度问题”。这一转化不仅使系统能够利用成熟的调度理论进行分析与优化，也为执行过程的可验证性与可控性提供了理论基础。

具体而言，在 Action Fabric 框架中，任务被表示为有向无环图，节点对应可执行动作，边表示依赖关系。调度器根据节点状态动态计算就绪集合，并在依赖满足时触发执行。与 Agent Loop 的单就绪结构不同，这一模型允许多个节点同时处于可执行状态，从而实现并行执行与路径选择。研究 ([arXiv, Apr 2026](https://arxiv.org/abs/2604.11378?utm_source=chatgpt.com)) 进一步指出，这种从 |U|=1 到 |U|>1 的跃迁并非简单的工程优化，而是执行能力的本质提升，因为它使系统具备了表达并发结构与替代路径的能力。

综上所述，Action Fabric 的理论依据建立在两个关键基础之上：其一是对 Agent Loop 的调度理论重构，其二是对 DAG 调度模型在 LLM 执行中的适用性扩展。前者揭示了现有系统的结构性瓶颈，后者提供了可行的替代路径。这种理论上的统一，使得 Action Fabric 不仅是一个工程方案，更是对智能体执行范式的一种系统性定义。

## 2. 技术依据

在工程实现层面，Action Fabric 已经落地为两层 Rust crate 的骨架：`dispatcher` 负责调度内核与恢复机制，`actions` 负责 Action 抽象与 Rust↔Kotlin 的执行桥。该实现将论文中的 ready set、状态机、三层分离与有界恢复直接映射到模块结构与核心类型上，便于后续扩展与替换。

结构化 Action 的实现方式如下：执行单元以 Action trait 表达，输入输出采用二进制 payload，便于跨语言序列化。Rust 侧可以直接执行本地 Action，也可通过 RemoteAction 转发到 Kotlin 侧服务，实现异步执行与跨进程隔离。

```rust
#[async_trait]
pub trait Action: Send + Sync {
  async fn execute(&self, input: ActionInput) -> ActionOutput;
}

pub struct ActionInput {
  pub payload: Vec<u8>,
}

pub struct ActionOutput {
  pub payload: Vec<u8>,
  pub error: Option<String>,
}
```

调度侧采用 DAG + 状态驱动引擎。节点状态机显式建模，ready set 由依赖与状态共同决定，执行引擎仅做“计算就绪集合→分发→应用状态转移”的确定性循环。

```rust
pub enum NodeState {
  Pending,
  Ready,
  Running,
  WaitingHuman,
  Blocked,
  Executed,
  FailedRetryable,
  Failed,
  Cancelled,
  Skipped,
}

pub fn compute_ready_set(state: &GlobalState, plan: &ExecutionPlan) -> Vec<NodeId> {
  plan.nodes
    .keys()
    .filter(|id| state.is_ready(id) && state.all_predecessors_executed(id, plan))
    .cloned()
    .collect()
}
```

执行引擎采用调度策略接口（默认 TopoPolicy），支持确定性“全就绪并发”。Recovery 层独立于执行上下文，仅接收失败记录并按层级升级，形成有界恢复链。

```rust
pub struct Engine {
  pub plan: ExecutionPlan,
  pub state: GlobalState,
  pub dispatcher: Dispatcher,
  pub executor: Box<dyn Executor>,
  pub recovery: Box<dyn RecoveryStrategy>,
}

pub async fn run(&mut self, context: &ExecutionContext) {
  loop {
    let ready = compute_ready_set(&self.state, &self.plan);
    if ready.is_empty() {
      break;
    }
    let to_run = self.dispatcher.dispatch_ready(ready);
    let results = self.executor.execute_batch(to_run, context).await;
    for result in results {
      self.apply_transition(result);
    }
  }
}
```

跨语言桥接采用 gRPC/IPC 协议，Kotlin 端暴露 ActionService，Rust 端以 GrpcClient 调用。该方案保证接口稳定、进程隔离与异步执行能力，是当前阶段最稳妥的技术选型。

```proto
service ActionService {
  rpc Execute (ActionRequest) returns (ActionResponse);
}

message ActionRequest {
  string action_name = 1;
  bytes payload = 2;
}

message ActionResponse {
  bytes result = 1;
  string error = 2;
}
```

Action 注册表将本地与远程 Action 统一到同一入口，调度器只需持有 Action 名称即可选择执行路径，避免执行层知晓跨语言细节。

```rust
pub struct ActionRegistry {
  local: HashMap<String, Arc<dyn Action>>,
  remote: HashMap<String, RemoteAction>,
}
```

从技术选型角度看，该实现体现了三点：一是调度内核使用 Rust 保证高并发与内存安全；二是执行抽象与恢复逻辑显式分层，降低运行时不确定性；三是跨语言执行通过 gRPC 保障可维护性与演进空间。这些选择与 Action Fabric 的设计原则一致，使其具备可验证、可扩展、可替换的工程基础。

## 3. 创新点

从技术演进的角度来看，Action Fabric 的创新并不体现在单一技术突破上，而在于对 Agent 执行范式的整体重构。其核心创新首先体现在执行控制机制的转变上。在传统 Agent Loop 中，执行路径由模型在运行过程中动态生成，这种机制虽然灵活，但缺乏稳定性与可控性。而 Action Fabric 将执行控制从模型推理中剥离，通过显式结构进行表达，使得执行行为从“隐式生成”转变为“显式调度”。这一转变意味着系统可以在执行前进行分析，在执行中进行控制，在执行后进行审计，从而具备完整的工程属性。

其次，Action Fabric 在执行语义层面引入了统一抽象。通过将工具调用提升为结构化动作，系统能够在统一框架下描述不同类型的操作，并为其赋予一致的执行规则。这种抽象不仅提高了系统的可组合性，也为后续调度与优化提供了基础。在传统系统中，不同工具之间缺乏统一语义，导致难以进行系统级优化，而在 Action Fabric 中，所有操作均以 Action 形式存在，从而形成统一执行模型。这一思想尽管在 Apple Shortcuts 等系统中已有体现，但仍然停留在用户侧自动化层面，其执行逻辑主要由用户手动构建，缺乏自动规划能力，同时其调度策略较为简单，以顺序执行为主，不支持并发，未形成完整调度体系。相比之下，Action Fabric 的目标是在系统层实现类似的动作抽象，但结合 LLM 的规划能力与 DAG 调度机制，使任务图能够自动生成并高效执行。因此，可以将 Action Fabric 视为对这类“动作流系统”的系统级扩展，即从“用户构建流程”演进为“模型生成并由系统调度执行的流程”。

再次，Action Fabric 通过显式依赖关系建模，使得任务执行过程具备可分析性。在 DAG 表示中，所有依赖关系均以图结构形式存在，这使得系统可以在执行前进行静态分析，例如检测循环依赖、评估执行路径以及估计资源消耗。这一能力在 Agent Loop 中是不存在的，因为其依赖关系仅存在于上下文中，无法被系统层直接访问。

此外，并发执行能力的引入是 Action Fabric 的重要创新之一。通过允许多个节点同时进入可执行状态，系统能够充分利用资源，从而显著提升执行效率。这种并发并非由模型决定，而是由结构保证，这使得并发行为具有稳定性与可预测性。这一点在复杂任务中尤为重要，因为其性能提升往往与可并行程度直接相关。

最后，在执行可解释性方面，Action Fabric 提供了结构化执行轨迹。由于每一步执行均对应于图中的节点，且状态变化由调度器统一管理，系统可以记录完整的执行过程。这种可追溯性使得系统在调试、审计以及安全控制方面具有显著优势，而这些能力在传统 Agent 系统中往往难以实现。

综上，Action Fabric 的创新可以概括为对执行控制、执行语义与执行结构的系统性统一，这种统一不仅提升了系统能力，也为其在复杂任务中的应用提供了基础。

## 4. 概要设计报告

### 4.1. 概述

在系统设计层面，Action Fabric 采用分层结构，将任务规划、执行调度与底层能力调用进行解耦。这一设计遵循“规划—执行—恢复分离”原则，使得不同功能模块之间具有清晰边界，从而提升系统的可维护性与可扩展性。

在执行流程上，系统首先接收用户输入的任务描述，并通过规划过程生成对应的任务图。该任务图以 DAG 形式表示，其中每个节点对应一个结构化动作，边表示依赖关系。随后，调度器根据图结构初始化节点状态，并计算初始就绪集合。在执行过程中，调度器不断根据节点完成情况更新就绪集合，并触发新的节点执行。这一过程持续进行，直到所有节点达到终态。

在节点执行阶段，每个 Action 根据其定义调用相应的底层能力，并生成输出结果。系统通过预定义的输出约束对结果进行验证，以确保其满足预期语义。如果执行结果不满足约束或出现错误，则触发恢复机制。恢复过程根据失败类型逐步进行，从简单重试到更复杂的调整，直至问题解决或达到终止条件。这一机制保证了系统在面对不确定性时的稳定性。

在执行控制方面，系统通过显式依赖关系确保执行顺序的正确性。只有当一个节点的所有前驱节点均完成时，该节点才会进入就绪集合。这种机制避免了由于上下文错误或模型失误导致的顺序错误，从而提升执行可靠性。同时，由于依赖关系在执行前已确定，系统可以在运行过程中避免不必要的等待，从而提升整体效率。

在执行结果管理方面，系统记录每个节点的状态变化与输出结果，从而形成完整的执行轨迹。这些信息不仅可以用于调试与分析，也可以用于后续优化。例如，通过分析执行时间分布，可以识别性能瓶颈；通过分析失败模式，可以改进恢复策略。这种数据驱动的优化能力，是 Action Fabric 相对于传统系统的重要优势之一。

总体而言，Action Fabric 的设计在不引入额外复杂机制的前提下，通过对执行结构的重构，实现了对执行过程的统一管理。其核心在于将执行从模型推理中解耦，使系统能够以类似传统计算系统的方式进行组织与调度。这一设计不仅在理论上具备合理性，在工程上也具有明确的实现路径，因此具有良好的可行性。

### 4.2 DAG 调度的核心形式化定义

#### 4.2.1 执行计划

一个执行计划是一个元组：
$$
\Pi = (id, version, V, E, \sigma, \kappa)
$$
其中：

- id：计划唯一标识符
- version：计划版本号，用于支持演化与回溯
- V：节点集合（set of nodes），表示所有执行任务
- $E \subseteq V \times V$：有向边集合，表示节点间依赖关系
- $\sigma : V \rightarrow NodeConfig$：节点配置函数，为每个节点分配执行配置，包括：
  - action 类型
  - 重试策略（retry policy）
  - 副作用级别（side-effect level）
  - 资源需求等
- $\kappa$：输出契约（output contract），定义执行完成后必须产出的结果结构

该定义强调三个关键点：

1. 执行结构显式化：任务不再隐含在 prompt 中，而是结构化为图
2. 执行语义可计算：每个节点具有明确 configuration，可被调度器解析
3. 输出可验证性：$\kappa$ 将“任务完成标准”从自然语言提升为结构约束

#### 4.2.2 Join 模式

##### 4.2.2.1 All-of Join

对于节点 v，其前驱集合为 P，当且仅当满足以下条件时，该节点进入 ready 状态：
$$
ready_{all\_of}(v) \iff \forall p \in P : \sigma(p) = executed
$$
含义：

- 只有当所有前驱节点均成功执行完成（executed）
- 当前节点才被调度器加入可执行队列（ready set）

适用场景：

- 多信息源融合（例如多文件分析后统一总结）
- 严格依赖型计算流程
- 数据一致性要求较高的任务

##### 4.2.2.2 Any-of Join

对于候选集合 C，其语义如下：

（1）分发规则（Dispatch）

- 所有依赖满足的候选节点 $c \in C$ 将被调度执行
- 执行顺序为确定性全序（deterministic total order），例如按 node id 升序
- 该顺序完全由 DAG 结构决定，与运行时状态无关

（2）成功传播（Success Propagation）

一旦存在某个候选节点 $c^* \in C$ 达到执行成功状态：
$$
ready_{any\_of}(v) \iff \exists c \in C : \sigma(c) = executed
$$
则：

- 节点 v 进入 ready 状态
- 表示该 join 已获得有效结果

（3）兄弟节点跳过（Sibling Skip）

当 $c^*$ 成功后：

- 所有尚未进入终态的兄弟节点 $C \setminus \{c^*\}$ 将被标记为 skipped
- 状态包括：pending / ready / running / retryable failed
- 已处于终态（executed / failed / cancelled）的节点保持不变

（4）失败传播（Failure Propagation）

如果所有候选节点均进入终态失败：

- 即所有 $c \in C$ 满足 failed 或 cancelled
- 则该 join 节点 v 进入 failed 状态

### 4.3 分层架构设计

#### 4.3.1 Execution Layer（执行层）

执行层负责执行 Planner (LLM) 生成的计划 $\Pi$，其核心特性包括：

- 输入为静态 DAG(V, E)
- 不允许修改图结构
- 维护每个节点状态（pending / ready / running / executed / failed / skipped）
- 动态计算 ready set
- 执行 Action 并记录 observation
- 向 Recovery Layer 上报失败信息

值得注意的是，执行层不做决策，只做调度与执行。即：

- 不决定如何修复
- 不修改 DAG
- 不重新规划结构

其本质是一个确定性调度执行引擎（deterministic execution engine）。

#### 4.3.2 Recovery Layer（恢复层）

恢复层是独立于执行循环的诊断与修复系统，其职责包括：

- 接收执行层上报的失败信息
- 诊断失败根因（root cause analysis）
- 从 Planner 申请并取回恢复策略（recovery action）

关键特点：

- 与执行上下文隔离
- 不直接参与 DAG 调度
- 基于“诊断上下文（diagnostic context）”决策
- 可触发：
  - 局部重试
  - 子图重规划
  - Patch DAG 插入

## 5. 结论

综合理论分析与技术论证可以看出，Action Fabric 的可行性建立在坚实的基础之上。其通过引入结构化动作与 DAG 调度机制，从根本上解决了 Agent Loop 在依赖表达、执行控制与错误恢复方面的结构性问题。同时，该方案并未依赖尚未验证的新技术，而是基于已有理论与工程实践进行整合与提升，因此在实现上具有较高可行性。

更重要的是，Action Fabric 所代表的不仅是一种具体实现方案，而是一种执行范式的转变。这一转变将 Agent 系统从以模型为中心的推理系统，转化为以调度为核心的执行系统，使其具备更强的可控性、可扩展性与工程适用性。在未来 Agent 系统持续向复杂任务演进的过程中，这一方向具有重要的理论价值与实践意义。
use actions::{Action, ActionInput, ActionOutput, ActionRegistry};
use async_trait::async_trait;
use dispatcher::scheduler::{Dispatcher, TopoPolicy};
use dispatcher::{
    ActionExecutor, ActionPolicy, Contract, Edge, Engine, ExecutionContext, ExecutionPlan,
    GlobalState, InMemoryAuditLog, InMemoryStateStore, Node, NodeConfig, RiskLevel,
    SideEffectLevel, SimpleRecovery,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use tokio::time::sleep;

const BASE_PROMPT_BYTES: u64 = 4_096;
const USER_TASK_BYTES: u64 = 768;
const NODE_SCHEMA_BYTES: u64 = 160;
const EDGE_SCHEMA_BYTES: u64 = 48;
const COMPACT_OBSERVATION_BYTES: u64 = 2_048;
const DEFAULT_ITERATIONS: usize = 5;
const TOOL_DELAY_MS: u64 = 8;
const RESULTS_DIR: &str = "benchmark_results/action_graph_vs_loop";

#[derive(Clone, Copy, Debug)]
struct Profile {
    name: &'static str,
    llm_delay_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum RunnerKind {
    Dag,
    AgentLoopFull,
    AgentLoopCompact,
    HeavySubagent,
}

impl RunnerKind {
    fn as_str(self) -> &'static str {
        match self {
            RunnerKind::Dag => "action_graph",
            RunnerKind::AgentLoopFull => "agent_loop_full",
            RunnerKind::AgentLoopCompact => "agent_loop_compact",
            RunnerKind::HeavySubagent => "heavy_subagent",
        }
    }
}

#[derive(Clone)]
struct Workload {
    name: String,
    family: &'static str,
    nodes: Vec<NodeSpec>,
    subtask_count: usize,
    subtask_steps: usize,
}

#[derive(Clone)]
struct NodeSpec {
    id: String,
    deps: Vec<String>,
    output_bytes: usize,
    fail_once: bool,
    retry_budget: u32,
}

#[derive(Clone, Debug)]
struct BenchRecord {
    scenario: String,
    family: &'static str,
    profile: &'static str,
    runner: RunnerKind,
    iteration: usize,
    success: bool,
    total_ms: f64,
    llm_calls: u64,
    tool_calls: u64,
    prompt_bytes_total: u64,
    max_context_bytes: u64,
    stored_output_bytes: u64,
    max_parallel_width: usize,
    failures: u64,
}

#[derive(Clone, Debug)]
struct Summary {
    scenario: String,
    family: &'static str,
    profile: &'static str,
    runner: RunnerKind,
    runs: usize,
    success_rate: f64,
    avg_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    avg_llm_calls: f64,
    avg_tool_calls: f64,
    avg_prompt_bytes: f64,
    avg_max_context_bytes: f64,
    avg_stored_output_bytes: f64,
    avg_parallel_width: f64,
    avg_failures: f64,
}

#[derive(Default)]
struct RunCounters {
    tool_calls: AtomicU64,
    stored_output_bytes: AtomicU64,
    failures: AtomicU64,
    current_parallel: AtomicUsize,
    max_parallel: AtomicUsize,
    attempts: Mutex<HashMap<String, u32>>,
}

impl RunCounters {
    fn enter_action(&self) {
        self.tool_calls.fetch_add(1, Ordering::Relaxed);
        let current = self.current_parallel.fetch_add(1, Ordering::Relaxed) + 1;
        let mut observed = self.max_parallel.load(Ordering::Relaxed);
        while current > observed {
            match self.max_parallel.compare_exchange(
                observed,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => observed = next,
            }
        }
    }

    fn leave_action(&self) {
        self.current_parallel.fetch_sub(1, Ordering::Relaxed);
    }

    fn next_attempt(&self, node_id: &str) -> u32 {
        let mut attempts = self.attempts.lock().expect("attempt counter lock");
        let entry = attempts.entry(node_id.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }
}

#[derive(Clone)]
struct BenchAction {
    counters: Arc<RunCounters>,
    tool_delay_ms: u64,
}

#[async_trait]
impl Action for BenchAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        self.counters.enter_action();
        let value = serde_json::from_slice::<Value>(&input.payload).unwrap_or_else(|_| json!({}));
        let node_id = value
            .get("node")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let output_bytes = value
            .get("outputBytes")
            .and_then(Value::as_u64)
            .unwrap_or(128) as usize;
        let fail_once = value
            .get("failOnce")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let attempt = self.counters.next_attempt(&node_id);

        sleep(Duration::from_millis(self.tool_delay_ms)).await;
        let output = if fail_once && attempt == 1 {
            self.counters.failures.fetch_add(1, Ordering::Relaxed);
            ActionOutput {
                payload: Vec::new(),
                error: Some(actions::ActionError::new_with(
                    "TRANSIENT_TOOL_FAILURE",
                    format!("{node_id} failed on first attempt"),
                    true,
                )),
            }
        } else {
            let payload = make_payload(&node_id, output_bytes);
            self.counters
                .stored_output_bytes
                .fetch_add(payload.len() as u64, Ordering::Relaxed);
            ActionOutput {
                payload,
                error: None,
            }
        };
        self.counters.leave_action();
        output
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iterations = parse_iterations();
    let profiles = [
        Profile {
            name: "edge_fast_40ms",
            llm_delay_ms: 40,
        },
        Profile {
            name: "edge_mid_120ms",
            llm_delay_ms: 120,
        },
        Profile {
            name: "edge_slow_300ms",
            llm_delay_ms: 300,
        },
    ];
    let workloads = build_workloads();
    let mut records = Vec::new();

    for profile in profiles {
        for workload in &workloads {
            let runners = runners_for(workload);
            for runner in runners {
                for iteration in 0..iterations {
                    let record = match runner {
                        RunnerKind::Dag => run_dag(workload, profile, iteration).await,
                        RunnerKind::AgentLoopFull => {
                            run_agent_loop(workload, profile, iteration, ObservationMode::Full)
                                .await
                        }
                        RunnerKind::AgentLoopCompact => {
                            run_agent_loop(workload, profile, iteration, ObservationMode::Compact)
                                .await
                        }
                        RunnerKind::HeavySubagent => {
                            run_heavy_subagent(workload, profile, iteration).await
                        }
                    };
                    records.push(record);
                }
            }
        }
    }

    let summaries = summarize(&records);
    fs::create_dir_all(RESULTS_DIR)?;
    write_raw_csv(&records)?;
    write_summary_csv(&summaries)?;
    write_analysis(&summaries, iterations)?;

    println!(
        "benchmark complete: {} runs, {} grouped summaries",
        records.len(),
        summaries.len()
    );
    println!("{RESULTS_DIR}/raw.csv");
    println!("{RESULTS_DIR}/summary.csv");
    println!("{RESULTS_DIR}/analysis.md");
    Ok(())
}

fn parse_iterations() -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .find_map(|pair| {
            (pair[0] == "--iterations")
                .then(|| pair[1].parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn build_workloads() -> Vec<Workload> {
    let mut workloads = Vec::new();
    for nodes in [4usize, 8, 12] {
        workloads.push(pipeline_workload(nodes, 256, false, 0));
    }
    for width in [4usize, 8, 16] {
        workloads.push(fanout_workload(width, 256));
    }
    for bytes in [16 * 1024usize, 64 * 1024, 256 * 1024, 1024 * 1024] {
        workloads.push(
            pipeline_workload(5, bytes, false, 0)
                .renamed("context_pressure", format!("context_5x_{}kb", bytes / 1024)),
        );
    }
    for rate in [0usize, 5, 20] {
        workloads.push(failure_workload(8, rate));
    }
    workloads.push(subagent_workload(5, 3));
    workloads
}

trait RenameWorkload {
    fn renamed(self, family: &'static str, name: String) -> Self;
}

impl RenameWorkload for Workload {
    fn renamed(mut self, family: &'static str, name: String) -> Self {
        self.family = family;
        self.name = name;
        self
    }
}

fn pipeline_workload(
    nodes: usize,
    output_bytes: usize,
    fail_once: bool,
    retry_budget: u32,
) -> Workload {
    let mut specs = Vec::new();
    for index in 0..nodes {
        let id = format!("step_{}", index + 1);
        let deps = if index == 0 {
            Vec::new()
        } else {
            vec![format!("step_{index}")]
        };
        specs.push(NodeSpec {
            id,
            deps,
            output_bytes,
            fail_once,
            retry_budget,
        });
    }
    Workload {
        name: format!("pipeline_{nodes}"),
        family: "deterministic_pipeline",
        nodes: specs,
        subtask_count: 0,
        subtask_steps: 0,
    }
}

fn fanout_workload(width: usize, output_bytes: usize) -> Workload {
    let mut specs = Vec::new();
    for index in 0..width {
        specs.push(NodeSpec {
            id: format!("read_{}", index + 1),
            deps: Vec::new(),
            output_bytes,
            fail_once: false,
            retry_budget: 0,
        });
    }
    specs.push(NodeSpec {
        id: "merge".to_string(),
        deps: (1..=width).map(|index| format!("read_{index}")).collect(),
        output_bytes,
        fail_once: false,
        retry_budget: 0,
    });
    Workload {
        name: format!("fanout_{width}"),
        family: "parallel_fanout",
        nodes: specs,
        subtask_count: 0,
        subtask_steps: 0,
    }
}

fn failure_workload(nodes: usize, rate_percent: usize) -> Workload {
    let mut workload = pipeline_workload(nodes, 256, false, 1)
        .renamed("failure_recovery", format!("failure_{}pct", rate_percent));
    if rate_percent > 0 {
        let fail_count = ((nodes * rate_percent) + 99) / 100;
        for index in 0..fail_count.max(1).min(nodes) {
            if let Some(node) = workload.nodes.get_mut(index * nodes / fail_count.max(1)) {
                node.fail_once = true;
            }
        }
    }
    workload
}

fn subagent_workload(subtasks: usize, steps_per_subtask: usize) -> Workload {
    let mut specs = Vec::new();
    for subtask in 0..subtasks {
        for step in 0..steps_per_subtask {
            let id = format!("task_{}_step_{}", subtask + 1, step + 1);
            let deps = if step == 0 {
                Vec::new()
            } else {
                vec![format!("task_{}_step_{}", subtask + 1, step)]
            };
            specs.push(NodeSpec {
                id,
                deps,
                output_bytes: 256,
                fail_once: false,
                retry_budget: 0,
            });
        }
    }
    specs.push(NodeSpec {
        id: "final_merge".to_string(),
        deps: (0..subtasks)
            .map(|subtask| format!("task_{}_step_{}", subtask + 1, steps_per_subtask))
            .collect(),
        output_bytes: 512,
        fail_once: false,
        retry_budget: 0,
    });
    Workload {
        name: format!("subtasks_{}x{}", subtasks, steps_per_subtask),
        family: "subagent_decomposition",
        nodes: specs,
        subtask_count: subtasks,
        subtask_steps: steps_per_subtask,
    }
}

fn runners_for(workload: &Workload) -> Vec<RunnerKind> {
    if workload.family == "subagent_decomposition" {
        vec![
            RunnerKind::Dag,
            RunnerKind::AgentLoopCompact,
            RunnerKind::HeavySubagent,
        ]
    } else {
        vec![
            RunnerKind::Dag,
            RunnerKind::AgentLoopFull,
            RunnerKind::AgentLoopCompact,
        ]
    }
}

async fn run_dag(workload: &Workload, profile: Profile, iteration: usize) -> BenchRecord {
    let counters = Arc::new(RunCounters::default());
    let mut registry = ActionRegistry::default();
    registry.register_local(
        "bench_action",
        Arc::new(BenchAction {
            counters: Arc::clone(&counters),
            tool_delay_ms: TOOL_DELAY_MS,
        }),
    );
    let registry = Arc::new(registry);
    let plan = build_plan(workload);
    let executor = ActionExecutor::new(Arc::clone(&registry), Arc::new(plan.clone()));
    let start = Instant::now();
    sleep(Duration::from_millis(profile.llm_delay_ms)).await;
    let mut engine = Engine {
        state: GlobalState::new(&plan),
        dispatcher: Dispatcher::new(Box::new(TopoPolicy)),
        executor: Box::new(executor),
        recovery: Box::new(SimpleRecovery::default()),
        audit_log: Box::new(InMemoryAuditLog::default()),
        state_store: Box::new(InMemoryStateStore::default()),
        diagnostic: Default::default(),
        plan,
    };
    let _ = engine.run(&ExecutionContext::default()).await;
    let success = engine
        .state
        .nodes
        .values()
        .all(|state| *state == dispatcher::NodeState::Executed);

    BenchRecord {
        scenario: workload.name.clone(),
        family: workload.family,
        profile: profile.name,
        runner: RunnerKind::Dag,
        iteration,
        success,
        total_ms: start.elapsed().as_secs_f64() * 1000.0,
        llm_calls: 1,
        tool_calls: counters.tool_calls.load(Ordering::Relaxed),
        prompt_bytes_total: planning_prompt_bytes(workload),
        max_context_bytes: planning_prompt_bytes(workload),
        stored_output_bytes: counters.stored_output_bytes.load(Ordering::Relaxed),
        max_parallel_width: counters.max_parallel.load(Ordering::Relaxed),
        failures: counters.failures.load(Ordering::Relaxed),
    }
}

#[derive(Clone, Copy)]
enum ObservationMode {
    Full,
    Compact,
}

async fn run_agent_loop(
    workload: &Workload,
    profile: Profile,
    iteration: usize,
    mode: ObservationMode,
) -> BenchRecord {
    let counters = Arc::new(RunCounters::default());
    let action = BenchAction {
        counters: Arc::clone(&counters),
        tool_delay_ms: TOOL_DELAY_MS,
    };
    let start = Instant::now();
    let mut outputs: HashMap<String, Vec<u8>> = HashMap::new();
    let mut history_bytes = BASE_PROMPT_BYTES + USER_TASK_BYTES + workload.nodes.len() as u64 * 96;
    let mut prompt_bytes_total = 0u64;
    let mut max_context_bytes = history_bytes;
    let mut llm_calls = 0u64;
    let mut success = true;

    for node in &workload.nodes {
        let mut attempts = 0u32;
        loop {
            llm_calls += 1;
            prompt_bytes_total += history_bytes;
            max_context_bytes = max_context_bytes.max(history_bytes);
            sleep(Duration::from_millis(profile.llm_delay_ms)).await;

            let input = ActionInput::new(loop_payload(node, &outputs));
            let output = action.execute(input).await;
            attempts += 1;
            match output.error {
                Some(error) if attempts <= node.retry_budget + 1 && error.retryable => {
                    let observation = error.message.len() as u64 + 128;
                    history_bytes += observation_for_mode(observation, mode);
                    continue;
                }
                Some(error) => {
                    success = false;
                    history_bytes += observation_for_mode(error.message.len() as u64 + 128, mode);
                    break;
                }
                None => {
                    let observation = output.payload.len() as u64;
                    history_bytes += observation_for_mode(observation, mode);
                    outputs.insert(node.id.clone(), output.payload);
                    break;
                }
            }
        }
        if !success {
            break;
        }
    }

    BenchRecord {
        scenario: workload.name.clone(),
        family: workload.family,
        profile: profile.name,
        runner: match mode {
            ObservationMode::Full => RunnerKind::AgentLoopFull,
            ObservationMode::Compact => RunnerKind::AgentLoopCompact,
        },
        iteration,
        success,
        total_ms: start.elapsed().as_secs_f64() * 1000.0,
        llm_calls,
        tool_calls: counters.tool_calls.load(Ordering::Relaxed),
        prompt_bytes_total,
        max_context_bytes,
        stored_output_bytes: counters.stored_output_bytes.load(Ordering::Relaxed),
        max_parallel_width: counters.max_parallel.load(Ordering::Relaxed),
        failures: counters.failures.load(Ordering::Relaxed),
    }
}

async fn run_heavy_subagent(
    workload: &Workload,
    profile: Profile,
    iteration: usize,
) -> BenchRecord {
    let counters = Arc::new(RunCounters::default());
    let start = Instant::now();
    let root_context = BASE_PROMPT_BYTES + USER_TASK_BYTES + workload.subtask_count as u64 * 512;
    sleep(Duration::from_millis(profile.llm_delay_ms)).await;

    let mut tasks = JoinSet::new();
    for subtask in 0..workload.subtask_count {
        let counters = Arc::clone(&counters);
        let profile_name = profile.name;
        let delay = profile.llm_delay_ms;
        let steps = workload.subtask_steps;
        tasks.spawn(async move {
            let action = BenchAction {
                counters,
                tool_delay_ms: TOOL_DELAY_MS,
            };
            let mut history_bytes = BASE_PROMPT_BYTES + USER_TASK_BYTES + 1_024;
            let mut prompt_bytes_total = 0u64;
            let mut max_context_bytes = history_bytes;
            let mut llm_calls = 0u64;
            let mut outputs = HashMap::new();
            for step in 0..steps {
                llm_calls += 1;
                prompt_bytes_total += history_bytes;
                max_context_bytes = max_context_bytes.max(history_bytes);
                sleep(Duration::from_millis(delay)).await;
                let id = format!("task_{}_step_{}", subtask + 1, step + 1);
                let deps = if step == 0 {
                    Vec::new()
                } else {
                    vec![format!("task_{}_step_{}", subtask + 1, step)]
                };
                let node = NodeSpec {
                    id: id.clone(),
                    deps,
                    output_bytes: 256,
                    fail_once: false,
                    retry_budget: 0,
                };
                let output = action
                    .execute(ActionInput::new(loop_payload(&node, &outputs)))
                    .await;
                history_bytes += output.payload.len() as u64;
                outputs.insert(id, output.payload);
            }
            let _ = profile_name;
            (llm_calls, prompt_bytes_total, max_context_bytes)
        });
    }

    let mut llm_calls = 1u64;
    let mut prompt_bytes_total = root_context;
    let mut max_context_bytes = root_context;
    while let Some(result) = tasks.join_next().await {
        if let Ok((calls, prompt_bytes, max_ctx)) = result {
            llm_calls += calls;
            prompt_bytes_total += prompt_bytes;
            max_context_bytes = max_context_bytes.max(max_ctx);
        }
    }
    sleep(Duration::from_millis(profile.llm_delay_ms)).await;
    llm_calls += 1;
    prompt_bytes_total += root_context;

    BenchRecord {
        scenario: workload.name.clone(),
        family: workload.family,
        profile: profile.name,
        runner: RunnerKind::HeavySubagent,
        iteration,
        success: true,
        total_ms: start.elapsed().as_secs_f64() * 1000.0,
        llm_calls,
        tool_calls: counters.tool_calls.load(Ordering::Relaxed),
        prompt_bytes_total,
        max_context_bytes,
        stored_output_bytes: counters.stored_output_bytes.load(Ordering::Relaxed),
        max_parallel_width: counters.max_parallel.load(Ordering::Relaxed),
        failures: counters.failures.load(Ordering::Relaxed),
    }
}

fn build_plan(workload: &Workload) -> ExecutionPlan {
    let mut nodes = HashMap::new();
    let mut edges = Vec::new();
    for spec in &workload.nodes {
        for dep in &spec.deps {
            edges.push(Edge {
                from: dep.clone(),
                to: spec.id.clone(),
            });
        }
        nodes.insert(
            spec.id.clone(),
            Node {
                id: spec.id.clone(),
                action: "bench_action".to_string(),
                inputs: Some(node_inputs(spec)),
                config: NodeConfig {
                    retry_budget: spec.retry_budget,
                    timeout: Duration::from_secs(30),
                    side_effect: SideEffectLevel::Pure,
                    policy: ActionPolicy::default()
                        .with_risk(RiskLevel::Low)
                        .with_timeout(30_000)
                        .with_retries(spec.retry_budget),
                },
                contract: Contract {
                    schema: "bytes".to_string(),
                },
            },
        );
    }
    ExecutionPlan {
        id: workload.name.clone(),
        version: 1,
        nodes,
        edges,
        output_node: workload.nodes.last().map(|node| node.id.clone()),
        output_contract: Contract {
            schema: "bytes".to_string(),
        },
    }
}

fn node_inputs(spec: &NodeSpec) -> Value {
    let mut map = Map::new();
    map.insert("node".to_string(), Value::String(spec.id.clone()));
    map.insert(
        "outputBytes".to_string(),
        Value::Number((spec.output_bytes as u64).into()),
    );
    map.insert("failOnce".to_string(), Value::Bool(spec.fail_once));
    for dep in &spec.deps {
        map.insert(format!("dep_{dep}"), Value::String(format!("${{{dep}}}")));
    }
    Value::Object(map)
}

fn loop_payload(spec: &NodeSpec, outputs: &HashMap<String, Vec<u8>>) -> Vec<u8> {
    let mut map = Map::new();
    map.insert("node".to_string(), Value::String(spec.id.clone()));
    map.insert(
        "outputBytes".to_string(),
        Value::Number((spec.output_bytes as u64).into()),
    );
    map.insert("failOnce".to_string(), Value::Bool(spec.fail_once));
    for dep in &spec.deps {
        let value = outputs
            .get(dep)
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .unwrap_or_default();
        map.insert(format!("dep_{dep}"), Value::String(value));
    }
    serde_json::to_vec(&Value::Object(map)).expect("benchmark payload should serialize")
}

fn planning_prompt_bytes(workload: &Workload) -> u64 {
    BASE_PROMPT_BYTES
        + USER_TASK_BYTES
        + workload.nodes.len() as u64 * NODE_SCHEMA_BYTES
        + edge_count(workload) as u64 * EDGE_SCHEMA_BYTES
}

fn edge_count(workload: &Workload) -> usize {
    workload.nodes.iter().map(|node| node.deps.len()).sum()
}

fn observation_for_mode(bytes: u64, mode: ObservationMode) -> u64 {
    match mode {
        ObservationMode::Full => bytes,
        ObservationMode::Compact => bytes.min(COMPACT_OBSERVATION_BYTES),
    }
}

fn make_payload(node_id: &str, bytes: usize) -> Vec<u8> {
    let prefix = format!("{node_id}:");
    if bytes <= prefix.len() {
        return prefix.into_bytes()[..bytes].to_vec();
    }
    let mut payload = Vec::with_capacity(bytes);
    payload.extend_from_slice(prefix.as_bytes());
    payload.resize(bytes, b'x');
    payload
}

fn summarize(records: &[BenchRecord]) -> Vec<Summary> {
    let mut groups: BTreeMap<(String, &'static str, &'static str, RunnerKind), Vec<&BenchRecord>> =
        BTreeMap::new();
    for record in records {
        groups
            .entry((
                record.scenario.clone(),
                record.family,
                record.profile,
                record.runner,
            ))
            .or_default()
            .push(record);
    }

    let mut summaries = Vec::new();
    for ((scenario, family, profile, runner), rows) in groups {
        let total_ms = rows.iter().map(|row| row.total_ms).collect::<Vec<_>>();
        let runs = rows.len();
        let success_count = rows.iter().filter(|row| row.success).count();
        summaries.push(Summary {
            scenario,
            family,
            profile,
            runner,
            runs,
            success_rate: success_count as f64 / runs as f64,
            avg_ms: avg_f64(&total_ms),
            p50_ms: percentile(total_ms.clone(), 0.50),
            p95_ms: percentile(total_ms, 0.95),
            avg_llm_calls: avg_u64(rows.iter().map(|row| row.llm_calls)),
            avg_tool_calls: avg_u64(rows.iter().map(|row| row.tool_calls)),
            avg_prompt_bytes: avg_u64(rows.iter().map(|row| row.prompt_bytes_total)),
            avg_max_context_bytes: avg_u64(rows.iter().map(|row| row.max_context_bytes)),
            avg_stored_output_bytes: avg_u64(rows.iter().map(|row| row.stored_output_bytes)),
            avg_parallel_width: avg_usize(rows.iter().map(|row| row.max_parallel_width)),
            avg_failures: avg_u64(rows.iter().map(|row| row.failures)),
        });
    }
    summaries
}

fn avg_f64(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len().max(1) as f64
}

fn avg_u64(values: impl Iterator<Item = u64>) -> f64 {
    let mut sum = 0u64;
    let mut count = 0u64;
    for value in values {
        sum += value;
        count += 1;
    }
    sum as f64 / count.max(1) as f64
}

fn avg_usize(values: impl Iterator<Item = usize>) -> f64 {
    let mut sum = 0usize;
    let mut count = 0usize;
    for value in values {
        sum += value;
        count += 1;
    }
    sum as f64 / count.max(1) as f64
}

fn percentile(mut values: Vec<f64>, p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let rank = ((values.len() - 1) as f64 * p).ceil() as usize;
    values[rank.min(values.len() - 1)]
}

fn write_raw_csv(records: &[BenchRecord]) -> std::io::Result<()> {
    let file = File::create(format!("{RESULTS_DIR}/raw.csv"))?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "scenario,family,profile,runner,iteration,success,total_ms,llm_calls,tool_calls,prompt_bytes_total,max_context_bytes,stored_output_bytes,max_parallel_width,failures"
    )?;
    for row in records {
        writeln!(
            writer,
            "{},{},{},{},{},{},{:.3},{},{},{},{},{},{},{}",
            row.scenario,
            row.family,
            row.profile,
            row.runner.as_str(),
            row.iteration,
            row.success,
            row.total_ms,
            row.llm_calls,
            row.tool_calls,
            row.prompt_bytes_total,
            row.max_context_bytes,
            row.stored_output_bytes,
            row.max_parallel_width,
            row.failures
        )?;
    }
    Ok(())
}

fn write_summary_csv(summaries: &[Summary]) -> std::io::Result<()> {
    let file = File::create(format!("{RESULTS_DIR}/summary.csv"))?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "scenario,family,profile,runner,runs,success_rate,avg_ms,p50_ms,p95_ms,avg_llm_calls,avg_tool_calls,avg_prompt_bytes,avg_max_context_bytes,avg_stored_output_bytes,avg_parallel_width,avg_failures"
    )?;
    for row in summaries {
        writeln!(
            writer,
            "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.2},{:.2},{:.0},{:.0},{:.0},{:.2},{:.2}",
            row.scenario,
            row.family,
            row.profile,
            row.runner.as_str(),
            row.runs,
            row.success_rate,
            row.avg_ms,
            row.p50_ms,
            row.p95_ms,
            row.avg_llm_calls,
            row.avg_tool_calls,
            row.avg_prompt_bytes,
            row.avg_max_context_bytes,
            row.avg_stored_output_bytes,
            row.avg_parallel_width,
            row.avg_failures
        )?;
    }
    Ok(())
}

fn write_analysis(summaries: &[Summary], iterations: usize) -> std::io::Result<()> {
    let file = File::create(format!("{RESULTS_DIR}/analysis.md"))?;
    let mut out = BufWriter::new(file);
    writeln!(out, "# Action Graph vs Agent Loop 本地性能 Benchmark")?;
    writeln!(out)?;
    writeln!(
        out,
        "本实验在本机对 Action Graph、传统 Agent Loop 和 Heavy Subagent 三种执行方式进行控制变量对比。每组配置重复 {iterations} 次，工具节点走与 dispatcher 相同的本地 Action 抽象，Action Graph 路径复用 `dispatcher::Engine`、ready set 调度和 `ActionExecutor::execute_batch`。"
    )?;
    writeln!(out)?;
    writeln!(out, "## 关键结论")?;
    writeln!(out)?;
    write_key_findings(&mut out, summaries)?;
    writeln!(out)?;
    writeln!(out, "## 代表性结果")?;
    writeln!(out)?;
    write_family_table(&mut out, summaries, "deterministic_pipeline", "pipeline_12")?;
    write_family_table(&mut out, summaries, "parallel_fanout", "fanout_16")?;
    write_family_table(&mut out, summaries, "context_pressure", "context_5x_1024kb")?;
    write_family_table(&mut out, summaries, "failure_recovery", "failure_20pct")?;
    write_family_table(
        &mut out,
        summaries,
        "subagent_decomposition",
        "subtasks_5x3",
    )?;
    writeln!(out, "## 指标说明")?;
    writeln!(out)?;
    writeln!(
        out,
        "- `avg_ms` / `p95_ms`：端到端 wall-clock 延迟，包含规划等待、工具执行和调度开销。"
    )?;
    writeln!(
        out,
        "- `avg_llm_calls`：一次任务需要进入语言模型决策的平均次数。"
    )?;
    writeln!(
        out,
        "- `avg_prompt_bytes`：所有模型轮次输入上下文总量，反映 token 成本压力。"
    )?;
    writeln!(
        out,
        "- `avg_max_context_bytes`：单轮最大上下文大小，反映端侧小 context 风险。"
    )?;
    writeln!(
        out,
        "- `avg_parallel_width`：执行期观测到的最大并发工具节点数。"
    )?;
    Ok(())
}

fn write_key_findings(out: &mut BufWriter<File>, summaries: &[Summary]) -> std::io::Result<()> {
    if let Some((dag, loop_full)) = pair(summaries, "pipeline_12", "edge_slow_300ms") {
        writeln!(
            out,
            "- 确定性 12 步流水线中，Action Graph 平均 {:.1} ms，传统 Agent Loop 平均 {:.1} ms，延迟降低 {:.1}%；模型调用从 {:.1} 次降到 {:.1} 次。",
            dag.avg_ms,
            loop_full.avg_ms,
            improvement(loop_full.avg_ms, dag.avg_ms),
            loop_full.avg_llm_calls,
            dag.avg_llm_calls
        )?;
    }
    if let Some((dag, loop_full)) = pair(summaries, "fanout_16", "edge_slow_300ms") {
        writeln!(
            out,
            "- 16 路 fan-out/fan-in 中，Action Graph 平均 {:.1} ms，传统 Agent Loop 平均 {:.1} ms；最大并发宽度从 {:.1} 提升到 {:.1}。",
            dag.avg_ms, loop_full.avg_ms, loop_full.avg_parallel_width, dag.avg_parallel_width
        )?;
    }
    if let Some((dag, compact)) = pair_with_runner(
        summaries,
        "context_5x_1024kb",
        "edge_mid_120ms",
        RunnerKind::AgentLoopCompact,
    ) {
        writeln!(
            out,
            "- 5 步、每步 1MB 输出的上下文压力场景中，即使使用 compact observation，Agent Loop 的累计模型输入约 {:.1} KB，Action Graph 约 {:.1} KB。",
            compact.avg_prompt_bytes / 1024.0,
            dag.avg_prompt_bytes / 1024.0
        )?;
    }
    if let Some((dag, compact)) = pair_with_runner(
        summaries,
        "subtasks_5x3",
        "edge_slow_300ms",
        RunnerKind::HeavySubagent,
    ) {
        writeln!(
            out,
            "- 5 个子任务、每个 3 步的任务分解中，Heavy Subagent 平均需要 {:.1} 次模型调用，Action Graph 只需要 {:.1} 次；上下文输入量减少 {:.1}%。",
            compact.avg_llm_calls,
            dag.avg_llm_calls,
            improvement(compact.avg_prompt_bytes, dag.avg_prompt_bytes)
        )?;
    }
    Ok(())
}

fn write_family_table(
    out: &mut BufWriter<File>,
    summaries: &[Summary],
    family: &str,
    scenario: &str,
) -> std::io::Result<()> {
    writeln!(out, "### {family}: `{scenario}`")?;
    writeln!(out)?;
    writeln!(
        out,
        "| profile | runner | avg_ms | p95_ms | llm_calls | prompt_kb | max_ctx_kb | tool_calls | parallel_width |"
    )?;
    writeln!(
        out,
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for row in summaries
        .iter()
        .filter(|row| row.family == family && row.scenario == scenario)
    {
        writeln!(
            out,
            "| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |",
            row.profile,
            row.runner.as_str(),
            row.avg_ms,
            row.p95_ms,
            row.avg_llm_calls,
            row.avg_prompt_bytes / 1024.0,
            row.avg_max_context_bytes / 1024.0,
            row.avg_tool_calls,
            row.avg_parallel_width
        )?;
    }
    writeln!(out)?;
    Ok(())
}

fn pair<'a>(
    summaries: &'a [Summary],
    scenario: &str,
    profile: &str,
) -> Option<(&'a Summary, &'a Summary)> {
    pair_with_runner(summaries, scenario, profile, RunnerKind::AgentLoopFull)
}

fn pair_with_runner<'a>(
    summaries: &'a [Summary],
    scenario: &str,
    profile: &str,
    baseline: RunnerKind,
) -> Option<(&'a Summary, &'a Summary)> {
    let dag = summaries.iter().find(|row| {
        row.scenario == scenario && row.profile == profile && row.runner == RunnerKind::Dag
    })?;
    let baseline = summaries
        .iter()
        .find(|row| row.scenario == scenario && row.profile == profile && row.runner == baseline)?;
    Some((dag, baseline))
}

fn improvement(before: f64, after: f64) -> f64 {
    if before <= 0.0 {
        0.0
    } else {
        (before - after) * 100.0 / before
    }
}

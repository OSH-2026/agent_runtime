import { invoke } from "@tauri-apps/api/core";

const sampleWorkflow = `version: 1
id: device-summary-report
output: report
steps:
  - id: device
    action: device_info
    inputs:
      includeHardware: true
  - id: system
    action: system_info
    inputs:
      includeStorage: true
  - id: network
    action: network_status
    inputs:
      includeDetails: true
  - id: power
    action: power_status
    inputs:
      includeDetails: true
  - id: storage
    action: storage_info
    inputs:
      includeExternal: true
  - id: device_summary
    action: subagent
    inputs:
      prompt: "请将以下设备硬件信息总结成一段简洁中文，不要输出 YAML：\${device}"
      model: "local-model"
      base_url: "http://10.0.2.2:8080"
      max_turns: 2
      temperature: 0.2
  - id: system_summary
    action: subagent
    inputs:
      prompt: "请将以下系统与存储信息总结成一段简洁中文，不要输出 YAML。系统：\${system}；存储：\${storage}"
      model: "local-model"
      base_url: "http://10.0.2.2:8080"
      max_turns: 2
      temperature: 0.2
  - id: status_summary
    action: subagent
    inputs:
      prompt: "请将以下网络与电源状态总结成一段简洁中文，不要输出 YAML。网络：\${network}；电源：\${power}"
      model: "local-model"
      base_url: "http://10.0.2.2:8080"
      max_turns: 2
      temperature: 0.2
  - id: report
    action: text
    inputs:
      value: |
        \${device_summary}

        \${system_summary}

        \${status_summary}
outputContract: text
`;

interface AuditEntry {
  nodeId: string;
  from: string;
  to: string;
}

interface WorkflowResult {
  planId: string;
  success: boolean;
  nodeStates: Record<string, string>;
  outputs: Record<string, string>;
  audit: AuditEntry[];
  diagnostics: Array<{ nodeId: string; message: string }>;
}

const workflow = document.querySelector<HTMLTextAreaElement>("#workflow")!;
const grpcEndpoint = document.querySelector<HTMLInputElement>("#grpc-endpoint")!;
const workflowInput = document.querySelector<HTMLInputElement>("#workflow-input")!;
const runButton = document.querySelector<HTMLButtonElement>("#run-button")!;
const runLabel = document.querySelector<HTMLElement>("#run-label")!;
const status = document.querySelector<HTMLElement>("#status")!;
const message = document.querySelector<HTMLElement>("#message")!;
const resultContent = document.querySelector<HTMLElement>("#result-content")!;
const outputs = document.querySelector<HTMLElement>("#outputs")!;
const audit = document.querySelector<HTMLOListElement>("#audit")!;

workflow.value = sampleWorkflow;

document.querySelector("#reset-button")?.addEventListener("click", () => {
  workflow.value = sampleWorkflow;
  workflow.focus();
});

runButton.addEventListener("click", async () => {
  setRunning(true);
  try {
    const result = await invoke<WorkflowResult>("run_workflow", {
      yaml: workflow.value,
      input: workflowInput.value || null,
      grpcEndpoint: grpcEndpoint.value.trim() || null,
    });
    renderResult(result);
  } catch (error) {
    status.className = "status failure";
    status.textContent = "执行失败";
    message.textContent = String(error);
    resultContent.classList.add("hidden");
  } finally {
    setRunning(false);
  }
});

function setRunning(running: boolean) {
  runButton.disabled = running;
  runLabel.textContent = running ? "调度中..." : "运行 workflow";
  if (running) {
    status.className = "status running";
    status.textContent = "运行中";
    message.textContent = "Rust dispatcher 正在解析并执行 DAG。";
  }
}

function renderResult(result: WorkflowResult) {
  status.className = `status ${result.success ? "success" : "failure"}`;
  status.textContent = result.success ? "执行成功" : "未完全执行";
  message.textContent = `${result.planId} · ${Object.keys(result.nodeStates).length} 个节点`;
  resultContent.classList.remove("hidden");

  outputs.replaceChildren(
    ...Object.entries(result.nodeStates).map(([nodeId, nodeState]) => {
      const card = document.createElement("article");
      card.className = "output-card";

      const heading = document.createElement("div");
      heading.className = "output-heading";
      const name = document.createElement("strong");
      name.textContent = nodeId;
      const badge = document.createElement("span");
      badge.textContent = nodeState;
      heading.append(name, badge);

      const pre = document.createElement("pre");
      pre.textContent = result.outputs[nodeId] ?? "(无输出)";
      card.append(heading, pre);
      return card;
    }),
  );

  audit.replaceChildren(
    ...result.audit.map((entry) => {
      const item = document.createElement("li");
      item.textContent = `${entry.nodeId}: ${entry.from} → ${entry.to}`;
      return item;
    }),
  );

  for (const diagnostic of result.diagnostics) {
    const item = document.createElement("li");
    item.className = "diagnostic";
    item.textContent = `${diagnostic.nodeId}: ${diagnostic.message}`;
    audit.append(item);
  }
}

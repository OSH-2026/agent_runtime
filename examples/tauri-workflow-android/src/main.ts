import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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
  - id: system_summary
    action: subagent
    inputs:
      prompt: "请将以下系统与存储信息总结成一段简洁中文，不要输出 YAML。系统：\${system}；存储：\${storage}"
  - id: status_summary
    action: subagent
    inputs:
      prompt: "请将以下网络与电源状态总结成一段简洁中文，不要输出 YAML。网络：\${network}；电源：\${power}"
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

interface ConfirmationRequest {
  requestId: string;
  nodeId: string;
  action: string;
  inputs: unknown;
  risk: "low" | "medium" | "high" | "critical";
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
const confirmationOverlay = document.querySelector<HTMLElement>("#confirmation-overlay")!;
const confirmationAction = document.querySelector<HTMLElement>("#confirmation-action")!;
const confirmationMeta = document.querySelector<HTMLElement>("#confirmation-meta")!;
const confirmationInputs = document.querySelector<HTMLElement>("#confirmation-inputs")!;
const approveButton = document.querySelector<HTMLButtonElement>("#approve-button")!;
const rejectButton = document.querySelector<HTMLButtonElement>("#reject-button")!;

let pendingConfirmation: ConfirmationRequest | null = null;

workflow.value = sampleWorkflow;

const confirmationListenerReady = listen<ConfirmationRequest>(
  "confirmation-request",
  ({ payload }) => {
    pendingConfirmation = payload;
    confirmationAction.textContent = payload.action;
    confirmationMeta.textContent = `节点 ${payload.nodeId} · ${riskLabel(payload.risk)}`;
    confirmationInputs.textContent =
      payload.inputs == null ? "(无输入)" : JSON.stringify(payload.inputs, null, 2);
    confirmationOverlay.classList.remove("hidden");
    confirmationOverlay.setAttribute("aria-hidden", "false");
    setConfirmationButtons(false);
    status.className = "status waiting";
    status.textContent = "等待确认";
    message.textContent = `Action ${payload.action} 需要你的许可。`;
  },
);

document.querySelector("#reset-button")?.addEventListener("click", () => {
  workflow.value = sampleWorkflow;
  workflow.focus();
});

approveButton.addEventListener("click", () => {
  void resolvePendingConfirmation(true);
});

rejectButton.addEventListener("click", () => {
  void resolvePendingConfirmation(false);
});

runButton.addEventListener("click", async () => {
  setRunning(true);
  try {
    await confirmationListenerReady;
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

async function resolvePendingConfirmation(approved: boolean) {
  const confirmation = pendingConfirmation;
  if (!confirmation) {
    return;
  }
  setConfirmationButtons(true);
  try {
    await invoke("resolve_confirmation", {
      requestId: confirmation.requestId,
      approved,
    });
    pendingConfirmation = null;
    confirmationOverlay.classList.add("hidden");
    confirmationOverlay.setAttribute("aria-hidden", "true");
    status.className = approved ? "status running" : "status failure";
    status.textContent = approved ? "继续执行" : "已拒绝";
    message.textContent = approved
      ? `已允许 ${confirmation.action}，workflow 正在继续。`
      : `已拒绝 ${confirmation.action}。`;
  } catch (error) {
    message.textContent = `提交确认失败：${String(error)}`;
    setConfirmationButtons(false);
  }
}

function setConfirmationButtons(disabled: boolean) {
  approveButton.disabled = disabled;
  rejectButton.disabled = disabled;
}

function riskLabel(risk: ConfirmationRequest["risk"]) {
  const labels = {
    low: "低风险",
    medium: "中风险",
    high: "高风险",
    critical: "严重风险",
  };
  return labels[risk];
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

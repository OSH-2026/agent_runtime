import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  name?: string;
}

interface WorkflowReport {
  planId: string;
  success: boolean;
  outputNode: string | null;
  finalOutput: string | null;
  nodeStates: Record<string, string>;
  executedOutputs: Record<string, string>;
  diagnostics: Array<{ nodeId: string; message: string }>;
  error: string | null;
}

interface AgentStatusEvent {
  kind:
    | "thinking"
    | "workflow"
    | "executing"
    | "workflowSuccess"
    | "workflowFailure"
    | "complete";
  turn: number;
  message: string;
  yaml: string | null;
  workflow: WorkflowReport | null;
}

interface ChatLoopResponse {
  message: string;
  turns: number;
  workflows: WorkflowReport[];
}

interface ConfirmationRequest {
  requestId: string;
  nodeId: string;
  action: string;
  inputs: unknown;
  risk: "low" | "medium" | "high" | "critical";
}

const messages = element<HTMLDivElement>("#messages");
const composer = element<HTMLFormElement>("#composer");
const messageInput = element<HTMLTextAreaElement>("#message-input");
const sendButton = element<HTMLButtonElement>("#send-button");
const connectionLabel = element<HTMLElement>("#connection-label");
const activityList = element<HTMLOListElement>("#activity-list");
const activityEmpty = element<HTMLElement>("#activity-empty");
const settingsOverlay = element<HTMLElement>("#settings-overlay");
const confirmationOverlay = element<HTMLElement>("#confirmation-overlay");
const confirmationAction = element<HTMLElement>("#confirmation-action");
const confirmationMeta = element<HTMLElement>("#confirmation-meta");
const confirmationInputs = element<HTMLElement>("#confirmation-inputs");
const approveButton = element<HTMLButtonElement>("#approve-button");
const rejectButton = element<HTMLButtonElement>("#reject-button");

const history: ChatMessage[] = [];
let running = false;
let pendingConfirmation: ConfirmationRequest | null = null;
let activeActivity: HTMLLIElement | null = null;

const tauriAvailable = "__TAURI_INTERNALS__" in window;

if (tauriAvailable) {
  void listen<AgentStatusEvent>("agent-status", ({ payload }) => {
    renderAgentStatus(payload);
  });

  void listen<ConfirmationRequest>("confirmation-request", ({ payload }) => {
    pendingConfirmation = payload;
    confirmationAction.textContent = payload.action;
    confirmationMeta.textContent = `节点 ${payload.nodeId} · ${riskLabel(payload.risk)}`;
    confirmationInputs.textContent =
      payload.inputs == null ? "(无输入)" : JSON.stringify(payload.inputs, null, 2);
    showOverlay(confirmationOverlay);
    setDecisionDisabled(false);
    setConnection("等待确认", "waiting");
  });
} else {
  setConnection("浏览器预览", "waiting");
}

composer.addEventListener("submit", (event) => {
  event.preventDefault();
  void sendMessage();
});

messageInput.addEventListener("input", () => {
  messageInput.style.height = "auto";
  messageInput.style.height = `${Math.min(messageInput.scrollHeight, 144)}px`;
});

messageInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    composer.requestSubmit();
  }
});

document.querySelectorAll<HTMLButtonElement>(".suggestion").forEach((button) => {
  button.addEventListener("click", () => {
    messageInput.value = button.dataset.prompt ?? "";
    composer.requestSubmit();
  });
});

element<HTMLButtonElement>("#settings-button").addEventListener("click", () => {
  showOverlay(settingsOverlay);
});
element<HTMLButtonElement>("#close-settings").addEventListener("click", () => {
  hideOverlay(settingsOverlay);
});
element<HTMLButtonElement>("#save-settings").addEventListener("click", () => {
  persistSettings();
  hideOverlay(settingsOverlay);
});

approveButton.addEventListener("click", () => void resolveConfirmation(true));
rejectButton.addEventListener("click", () => void resolveConfirmation(false));

restoreSettings();

async function sendMessage() {
  const content = messageInput.value.trim();
  if (!content || running) {
    return;
  }

  appendMessage("user", content);
  messageInput.value = "";
  messageInput.style.height = "auto";
  setRunning(true);
  resetActivity();

  try {
    const response = await invoke<ChatLoopResponse>("run_chat_loop", {
      request: {
        message: content,
        history,
        config: readSettings(),
      },
    });
    history.push({ role: "user", content });
    history.push({ role: "assistant", content: response.message });
    appendMessage("assistant", response.message, response.turns, response.workflows.length);
    setConnection("已完成", "success");
  } catch (error) {
    appendError(String(error));
    setConnection("运行失败", "failure");
    finishActiveActivity("failure");
  } finally {
    setRunning(false);
  }
}

function renderAgentStatus(event: AgentStatusEvent) {
  activityEmpty.classList.add("hidden");

  if (event.kind === "thinking") {
    finishActiveActivity();
    activeActivity = createActivity(
      "thinking",
      `第 ${event.turn} 轮 · 模型`,
      "正在生成下一步…",
      true,
    );
    setConnection("模型思考中", "running");
    return;
  }

  if (event.kind === "workflow") {
    finishActiveActivity("success");
    activeActivity = createActivity(
      "workflow",
      `第 ${event.turn} 轮 · Workflow`,
      "已生成执行计划",
      false,
      event.yaml ?? undefined,
    );
    return;
  }

  if (event.kind === "executing") {
    finishActiveActivity("success");
    activeActivity = createActivity(
      "executing",
      `第 ${event.turn} 轮 · Dispatcher`,
      "正在调度节点…",
      true,
    );
    setConnection("执行 workflow", "running");
    return;
  }

  if (event.kind === "workflowSuccess" || event.kind === "workflowFailure") {
    finishActiveActivity(event.kind === "workflowSuccess" ? "success" : "failure");
    activeActivity = createWorkflowActivity(event);
    setConnection(
      event.kind === "workflowSuccess" ? "最终消息已返回" : "失败信息回传模型",
      event.kind === "workflowSuccess" ? "success" : "waiting",
    );
    return;
  }

  if (event.kind === "complete") {
    finishActiveActivity("success");
  }
}

function createActivity(
  type: string,
  title: string,
  description: string,
  loading: boolean,
  detail?: string,
) {
  const item = document.createElement("li");
  item.className = `activity-item ${type}`;
  item.innerHTML = `
    <span class="activity-node ${loading ? "pulse" : ""}"></span>
    <div class="activity-content">
      <div class="activity-title"><strong></strong><span></span></div>
      <p></p>
    </div>
  `;
  item.querySelector("strong")!.textContent = title;
  item.querySelector(".activity-title span")!.textContent = loading ? "运行中" : "已就绪";
  item.querySelector("p")!.textContent = description;
  if (detail) {
    const details = document.createElement("details");
    details.innerHTML = "<summary>查看 YAML</summary><pre></pre>";
    details.querySelector("pre")!.textContent = detail;
    item.querySelector(".activity-content")!.append(details);
  }
  activityList.append(item);
  scrollActivity();
  return item;
}

function createWorkflowActivity(event: AgentStatusEvent) {
  const report = event.workflow!;
  const item = createActivity(
    event.kind,
    `${report.planId} · ${Object.keys(report.nodeStates).length} 个节点`,
    event.message,
    false,
  );
  const badge = item.querySelector(".activity-title span")!;
  badge.textContent = report.success ? "成功" : "未完成";
  badge.className = report.success ? "success-text" : "failure-text";

  const nodeList = document.createElement("div");
  nodeList.className = "node-list";
  for (const [nodeId, state] of Object.entries(report.nodeStates)) {
    const node = document.createElement("div");
    node.className = "node-row";
    const output = report.executedOutputs[nodeId];
    node.innerHTML = `<span class="node-state ${state}"></span><strong></strong><small></small>`;
    node.querySelector("strong")!.textContent = nodeId;
    node.querySelector("small")!.textContent = stateLabel(state);
    if (output) {
      node.title = output;
    }
    nodeList.append(node);
  }
  item.querySelector(".activity-content")!.append(nodeList);

  if (report.error || report.diagnostics.length > 0) {
    const details = document.createElement("details");
    details.innerHTML = "<summary>失败详情</summary><pre></pre>";
    details.querySelector("pre")!.textContent = [
      report.error,
      ...report.diagnostics.map((entry) => `${entry.nodeId}: ${entry.message}`),
    ]
      .filter(Boolean)
      .join("\n");
    item.querySelector(".activity-content")!.append(details);
  }
  scrollActivity();
  return item;
}

function finishActiveActivity(result: "success" | "failure" = "success") {
  if (!activeActivity) {
    return;
  }
  activeActivity.querySelector(".activity-node")?.classList.remove("pulse");
  const state = activeActivity.querySelector(".activity-title span");
  if (state?.textContent === "运行中") {
    state.textContent = result === "success" ? "完成" : "失败";
    state.className = result === "success" ? "success-text" : "failure-text";
  }
  activeActivity = null;
}

function appendMessage(
  role: "user" | "assistant",
  content: string,
  turns?: number,
  workflows?: number,
) {
  const article = document.createElement("article");
  article.className = `message ${role}-message`;
  const avatar = document.createElement("div");
  avatar.className = "avatar";
  avatar.textContent = role === "assistant" ? "A" : "你";
  const body = document.createElement("div");
  body.className = "message-body";
  const paragraph = document.createElement("p");
  paragraph.textContent = content;
  body.append(paragraph);
  if (role === "assistant" && turns != null) {
    const meta = document.createElement("small");
    meta.className = "message-meta";
    meta.textContent =
      workflows && workflows > 0
        ? `${turns} 轮模型调用 · ${workflows} 个 workflow`
        : `${turns} 轮模型调用`;
    body.append(meta);
  }
  article.append(avatar, body);
  messages.append(article);
  messages.scrollTo({ top: messages.scrollHeight, behavior: "smooth" });
}

function appendError(content: string) {
  const article = document.createElement("article");
  article.className = "message system-message";
  article.textContent = `运行中断：${content}`;
  messages.append(article);
  messages.scrollTo({ top: messages.scrollHeight, behavior: "smooth" });
}

async function resolveConfirmation(approved: boolean) {
  const confirmation = pendingConfirmation;
  if (!confirmation) {
    return;
  }
  setDecisionDisabled(true);
  try {
    await invoke("resolve_confirmation", {
      requestId: confirmation.requestId,
      approved,
    });
    pendingConfirmation = null;
    hideOverlay(confirmationOverlay);
    setConnection(approved ? "继续执行" : "已拒绝操作", approved ? "running" : "waiting");
  } catch (error) {
    confirmationMeta.textContent = `提交决定失败：${String(error)}`;
    setDecisionDisabled(false);
  }
}

function setRunning(value: boolean) {
  running = value;
  sendButton.disabled = value;
  messageInput.disabled = value;
  sendButton.querySelector("span")!.textContent = value ? "运行中" : "发送";
  if (value) {
    setConnection("启动 agent loop", "running");
  }
}

function setConnection(label: string, state: string) {
  connectionLabel.textContent = label;
  connectionLabel.parentElement!.className = `connection-pill ${state}`;
}

function resetActivity() {
  activityList.replaceChildren();
  activityEmpty.classList.remove("hidden");
  activeActivity = null;
}

function readSettings() {
  return {
    modelBaseUrl: element<HTMLInputElement>("#model-url").value.trim(),
    model: element<HTMLInputElement>("#model-name").value.trim(),
    apiKey: element<HTMLInputElement>("#api-key").value.trim() || null,
    grpcEndpoint: element<HTMLInputElement>("#grpc-endpoint").value.trim(),
    temperature: Number(element<HTMLInputElement>("#temperature").value),
    maxTurns: Number(element<HTMLInputElement>("#max-turns").value),
  };
}

function persistSettings() {
  const settings = readSettings();
  localStorage.setItem(
    "action-chat-settings",
    JSON.stringify({ ...settings, apiKey: undefined }),
  );
}

function restoreSettings() {
  const raw = localStorage.getItem("action-chat-settings");
  if (!raw) {
    return;
  }
  try {
    const settings = JSON.parse(raw) as ReturnType<typeof readSettings>;
    setInput("#model-url", settings.modelBaseUrl);
    setInput("#model-name", settings.model);
    setInput("#grpc-endpoint", settings.grpcEndpoint);
    setInput("#temperature", String(settings.temperature));
    setInput("#max-turns", String(settings.maxTurns));
  } catch {
    localStorage.removeItem("action-chat-settings");
  }
}

function setInput(selector: string, value: string | undefined) {
  if (value) {
    element<HTMLInputElement>(selector).value = value;
  }
}

function showOverlay(overlay: HTMLElement) {
  overlay.classList.remove("hidden");
  overlay.setAttribute("aria-hidden", "false");
}

function hideOverlay(overlay: HTMLElement) {
  overlay.classList.add("hidden");
  overlay.setAttribute("aria-hidden", "true");
}

function setDecisionDisabled(value: boolean) {
  approveButton.disabled = value;
  rejectButton.disabled = value;
}

function scrollActivity() {
  activityList.parentElement?.scrollTo({
    top: activityList.parentElement.scrollHeight,
    behavior: "smooth",
  });
}

function riskLabel(risk: ConfirmationRequest["risk"]) {
  return {
    low: "低风险",
    medium: "中风险",
    high: "高风险",
    critical: "严重风险",
  }[risk];
}

function stateLabel(state: string) {
  return {
    pending: "等待",
    ready: "就绪",
    running: "执行中",
    waitingHuman: "待确认",
    blocked: "阻塞",
    executed: "成功",
    failedRetryable: "可重试",
    failed: "失败",
    cancelled: "已取消",
    skipped: "已跳过",
  }[state] ?? state;
}

function element<T extends Element>(selector: string): T {
  const value = document.querySelector<T>(selector);
  if (!value) {
    throw new Error(`missing element: ${selector}`);
  }
  return value;
}

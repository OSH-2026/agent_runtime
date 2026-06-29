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

type ConfirmationRisk = "low" | "medium" | "high" | "critical";

interface ConfirmationItem {
  nodeId: string;
  action: string;
  inputs: unknown;
  risk: ConfirmationRisk;
}

interface ConfirmationRequest {
  requestId: string;
  nodeId: string;
  action: string;
  inputs: unknown;
  risk: ConfirmationRisk;
  workflowId?: string | null;
  items?: ConfirmationItem[];
}

interface TraceSession {
  article: HTMLElement;
  details: HTMLDetailsElement;
  title: HTMLElement;
  status: HTMLElement;
  count: HTMLElement;
  list: HTMLOListElement;
  answer: HTMLParagraphElement;
  activeItem: HTMLLIElement | null;
  itemCount: number;
  hasFailure: boolean;
}

const messages = element<HTMLDivElement>("#messages");
const composer = element<HTMLFormElement>("#composer");
const messageInput = element<HTMLTextAreaElement>("#message-input");
const sendButton = element<HTMLButtonElement>("#send-button");
const connectionLabel = element<HTMLElement>("#connection-label");
const settingsOverlay = element<HTMLElement>("#settings-overlay");
const confirmationOverlay = element<HTMLElement>("#confirmation-overlay");
const confirmationRiskIcon = element<HTMLElement>("#confirmation-risk-icon");
const confirmationTitle = element<HTMLElement>("#confirmation-title");
const confirmationMeta = element<HTMLElement>("#confirmation-meta");
const confirmationSummary = element<HTMLElement>("#confirmation-summary");
const confirmationActionList = element<HTMLElement>("#confirmation-action-list");
const confirmationDetails = element<HTMLDetailsElement>("#confirmation-details");
const confirmationInputs = element<HTMLElement>("#confirmation-inputs");
const approveButton = element<HTMLButtonElement>("#approve-button");
const rejectButton = element<HTMLButtonElement>("#reject-button");

const history: ChatMessage[] = [];
let running = false;
let pendingConfirmation: ConfirmationRequest | null = null;
let activeTrace: TraceSession | null = null;

const tauriAvailable = "__TAURI_INTERNALS__" in window;

if (tauriAvailable) {
  void listen<AgentStatusEvent>("agent-status", ({ payload }) => {
    renderAgentStatus(payload);
  });

  void listen<ConfirmationRequest>("confirmation-request", ({ payload }) => {
    pendingConfirmation = payload;
    renderConfirmationRequest(payload);
    showOverlay(confirmationOverlay);
    setDecisionDisabled(false);
    setConnection(isBatchConfirmation(payload) ? "等待统一确认" : "等待确认", "waiting");
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
showPreviewIfRequested();

async function sendMessage() {
  const content = messageInput.value.trim();
  if (!content || running) {
    return;
  }

  appendMessage("user", content);
  const trace = createAssistantTurn();
  activeTrace = trace;
  messageInput.value = "";
  messageInput.style.height = "auto";
  setRunning(true);

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
    completeAssistantTurn(trace, response.message);
    setConnection("已完成", "success");
  } catch (error) {
    failAssistantTurn(trace, String(error));
    setConnection("运行失败", "failure");
  } finally {
    activeTrace = null;
    setRunning(false);
  }
}

function renderAgentStatus(event: AgentStatusEvent) {
  const trace = ensureActiveTrace();

  if (event.kind === "thinking") {
    finishActiveActivity(trace);
    trace.activeItem = createActivity(
      trace,
      "thinking",
      "思考",
      "正在生成下一步…",
      true,
    );
    setTraceStatus(trace, "正在思考", "running");
    setConnection("思考中", "running");
    return;
  }

  if (event.kind === "workflow") {
    finishActiveActivity(trace, "success");
    trace.activeItem = createActivity(
      trace,
      "workflow",
      "生成执行计划",
      "已生成执行计划",
      false,
      event.yaml ?? undefined,
    );
    setTraceStatus(trace, "已生成执行计划", "running");
    return;
  }

  if (event.kind === "executing") {
    finishActiveActivity(trace, "success");
    trace.activeItem = createActivity(
      trace,
      "executing",
      "执行任务",
      "正在执行步骤…",
      true,
    );
    setTraceStatus(trace, "正在执行任务", "running");
    setConnection("正在执行任务", "running");
    return;
  }

  if (event.kind === "workflowSuccess" || event.kind === "workflowFailure") {
    const result = event.kind === "workflowSuccess" ? "success" : "failure";
    finishActiveActivity(trace, result);
    trace.activeItem = createWorkflowActivity(trace, event);
    if (result === "failure") {
      trace.hasFailure = true;
      trace.details.open = true;
    }
    setTraceStatus(
      trace,
      result === "success" ? "执行完成" : "执行遇到问题",
      result,
    );
    setConnection(
      event.kind === "workflowSuccess" ? "最终回复已生成" : "正在修正任务",
      event.kind === "workflowSuccess" ? "success" : "waiting",
    );
    return;
  }

  if (event.kind === "complete") {
    finishActiveActivity(trace, "success");
    setTraceStatus(trace, "准备最终回复", "success");
  }
}

function createActivity(
  trace: TraceSession,
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
    details.innerHTML = "<summary>查看原始计划</summary><pre></pre>";
    details.querySelector("pre")!.textContent = detail;
    item.querySelector(".activity-content")!.append(details);
  }
  trace.list.append(item);
  trace.itemCount += 1;
  updateTraceCount(trace);
  scrollMessages();
  return item;
}

function createWorkflowActivity(trace: TraceSession, event: AgentStatusEvent) {
  const report = event.workflow!;
  const item = createActivity(
    trace,
    event.kind,
    `${report.planId} · ${Object.keys(report.nodeStates).length} 个步骤`,
    report.success ? "已完成" : cleanRuntimeMessage(event.message),
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
    details.innerHTML = report.success
      ? "<summary>诊断信息</summary><pre></pre>"
      : "<summary>原始错误信息</summary><pre></pre>";
    details.querySelector("pre")!.textContent = [
      report.error,
      ...report.diagnostics.map((entry) => `${entry.nodeId}: ${entry.message}`),
    ]
      .filter(Boolean)
      .join("\n");
    item.querySelector(".activity-content")!.append(details);
  }
  scrollMessages();
  return item;
}

function cleanRuntimeMessage(message: string) {
  return message
    .replace(/workflow/gi, "任务")
    .replace(/dispatcher/gi, "执行器")
    .replace(/node/gi, "步骤")
    .replace(/节点/g, "步骤");
}

function finishActiveActivity(
  trace = activeTrace,
  result: "success" | "failure" = "success",
) {
  if (!trace?.activeItem) {
    return;
  }
  trace.activeItem.querySelector(".activity-node")?.classList.remove("pulse");
  const state = trace.activeItem.querySelector(".activity-title span");
  if (state?.textContent === "运行中") {
    state.textContent = result === "success" ? "完成" : "失败";
    state.className = result === "success" ? "success-text" : "failure-text";
  }
  trace.activeItem = null;
}

function appendMessage(
  role: "user" | "assistant",
  content: string,
) {
  if (role === "assistant") {
    const trace = createAssistantTurn(false);
    completeAssistantTurn(trace, content);
    return;
  }

  const article = document.createElement("article");
  article.className = `message ${role}-message`;
  const avatar = document.createElement("div");
  setAvatarIcon(avatar, "user");
  const body = document.createElement("div");
  body.className = "message-body";
  const paragraph = document.createElement("p");
  paragraph.textContent = content;
  body.append(paragraph);
  article.append(avatar, body);
  messages.append(article);
  scrollMessages();
}

function createAssistantTurn(showThinking = true): TraceSession {
  const article = document.createElement("article");
  article.className = "message assistant-message";

  const avatar = document.createElement("div");
  setAvatarIcon(avatar, "bot");

  const body = document.createElement("div");
  body.className = "message-body";

  const details = document.createElement("details");
  details.className = "thinking-panel";
  details.open = true;
  details.innerHTML = `
    <summary>
      <span class="thinking-dot pulse"></span>
      <span class="thinking-copy">
        <strong>思考中</strong>
        <small>正在规划下一步</small>
      </span>
      <span class="thinking-count">0 步</span>
    </summary>
    <ol class="activity-list thinking-list"></ol>
  `;

  const answer = document.createElement("p");
  answer.className = "message-text hidden";

  if (showThinking) {
    body.append(details);
  } else {
    details.classList.add("hidden");
  }
  body.append(answer);
  article.append(avatar, body);
  messages.append(article);
  scrollMessages();

  return {
    article,
    details,
    title: details.querySelector(".thinking-copy strong")!,
    status: details.querySelector(".thinking-copy small")!,
    count: details.querySelector(".thinking-count")!,
    list: details.querySelector(".thinking-list")!,
    answer,
    activeItem: null,
    itemCount: 0,
    hasFailure: false,
  };
}

function ensureActiveTrace() {
  if (!activeTrace) {
    activeTrace = createAssistantTurn();
  }
  return activeTrace;
}

function setAvatarIcon(avatar: HTMLElement, kind: "bot" | "user") {
  avatar.className = `avatar avatar-${kind}`;
  avatar.setAttribute("aria-hidden", "true");
  avatar.innerHTML =
    kind === "bot"
      ? `<svg viewBox="0 0 24 24">
          <path d="M12 7V4"></path>
          <rect x="5" y="7" width="14" height="12" rx="4"></rect>
          <path d="M9 12h.01"></path>
          <path d="M15 12h.01"></path>
          <path d="M9 16h6"></path>
        </svg>`
      : `<svg viewBox="0 0 24 24">
          <circle cx="12" cy="8" r="4"></circle>
          <path d="M5 20a7 7 0 0 1 14 0"></path>
        </svg>`;
}

function completeAssistantTurn(trace: TraceSession, content: string) {
  finishActiveActivity(trace, trace.hasFailure ? "failure" : "success");
  trace.answer.textContent = content;
  trace.answer.classList.remove("hidden");
  trace.article.classList.remove("pending-message");

  if (trace.itemCount === 0) {
    trace.details.classList.add("hidden");
  } else {
    setTraceStatus(
      trace,
      trace.hasFailure ? "运行遇到问题" : "已完成",
      trace.hasFailure ? "failure" : "success",
    );
    trace.details.open = trace.hasFailure;
  }
  scrollMessages();
}

function failAssistantTurn(trace: TraceSession, error: string) {
  trace.hasFailure = true;
  finishActiveActivity(trace, "failure");
  setTraceStatus(trace, "运行失败", "failure");
  trace.details.open = true;
  trace.answer.textContent = `运行中断：${error}`;
  trace.answer.classList.remove("hidden");
  trace.answer.classList.add("error-text");
  scrollMessages();
}

function setTraceStatus(
  trace: TraceSession,
  label: string,
  state: "running" | "success" | "failure",
) {
  trace.title.textContent = state === "running" ? "思考中" : "运行过程";
  trace.status.textContent = label;
  trace.details.classList.toggle("running", state === "running");
  trace.details.classList.toggle("success", state === "success");
  trace.details.classList.toggle("failure", state === "failure");
  trace.details
    .querySelector(".thinking-dot")
    ?.classList.toggle("pulse", state === "running");
}

function updateTraceCount(trace: TraceSession) {
  trace.count.textContent = `${trace.itemCount} 步`;
}

function scrollMessages() {
  messages.scrollTo({ top: messages.scrollHeight, behavior: "smooth" });
}

function renderConfirmationRequest(payload: ConfirmationRequest) {
  const items = confirmationItems(payload);
  const batch = isBatchConfirmation(payload);
  const summary = buildConfirmationSummary(payload, items);

  confirmationRiskIcon.className = `risk-icon ${payload.risk}`;
  confirmationRiskIcon.textContent = riskIcon(payload.risk);
  confirmationTitle.textContent = summary.title;
  confirmationMeta.textContent = summary.meta;
  confirmationSummary.replaceChildren();
  confirmationSummary.append(createSummaryText(summary.headline, summary.description));
  confirmationActionList.replaceChildren(
    ...items.map((item, index) => createConfirmationItem(item, index, batch)),
  );
  confirmationDetails.open = payload.risk === "critical";
  confirmationInputs.textContent = formatTechnicalConfirmation(payload, items);
  rejectButton.textContent = "不允许";
  approveButton.textContent = batch ? `允许 ${items.length} 个操作` : "允许执行";
}

function showPreviewIfRequested() {
  const preview = new URLSearchParams(window.location.search).get("preview");
  if (tauriAvailable) {
    return;
  }

  if (preview === "thinking") {
    appendMessage("user", "查看当前设备、网络和电量状态，并给我一份简洁摘要。");
    const trace = createAssistantTurn();
    activeTrace = trace;
    renderAgentStatus({
      kind: "thinking",
      turn: 1,
      message: "正在理解请求",
      yaml: null,
      workflow: null,
    });
    renderAgentStatus({
      kind: "workflow",
      turn: 1,
      message: "已生成设备状态检查任务",
      yaml:
        "id: inspect-device\nnodes:\n  battery:\n    action: get_battery_status\n  network:\n    action: get_network_status\n  storage:\n    action: get_storage_status",
      workflow: null,
    });
    renderAgentStatus({
      kind: "executing",
      turn: 1,
      message: "正在执行步骤",
      yaml: null,
      workflow: null,
    });
    renderAgentStatus({
      kind: "workflowSuccess",
      turn: 1,
      message: "设备状态已读取完成",
      yaml: null,
      workflow: {
        planId: "inspect-device",
        success: true,
        outputNode: "summary",
        finalOutput: "设备状态正常",
        nodeStates: {
          battery: "executed",
          network: "executed",
          storage: "executed",
        },
        executedOutputs: {
          battery: "电量 78%，未充电",
          network: "Wi-Fi 已连接，信号良好",
          storage: "剩余空间充足",
        },
        diagnostics: [],
        error: null,
      },
    });
    completeAssistantTurn(
      trace,
      "设备状态看起来正常：电量 78%，Wi-Fi 连接稳定，存储空间充足。当前没有需要立即处理的问题。",
    );
    trace.details.open = true;
    activeTrace = null;
    setConnection("预览运行过程", "success");
    return;
  }

  if (preview === "recovered") {
    appendMessage("user", "读取剪贴板内容并告诉我重点。");
    const trace = createAssistantTurn();
    activeTrace = trace;
    renderAgentStatus({
      kind: "thinking",
      turn: 1,
      message: "正在理解请求",
      yaml: null,
      workflow: null,
    });
    renderAgentStatus({
      kind: "workflow",
      turn: 1,
      message: "已生成剪贴板总结任务",
      yaml:
        "version: 1\nid: read-clipboard-summary\nsteps:\n  - id: clip_data\n    action: clipboard_read\n    inputs:\n      unused: true\n  - id: final_report\n    action: subagent\n    inputs:\n      prompt: \"读取以下剪贴板内容，用中文总结核心重点。内容：${clip_data}\"",
      workflow: null,
    });
    renderAgentStatus({
      kind: "executing",
      turn: 1,
      message: "正在执行步骤",
      yaml: null,
      workflow: null,
    });
    renderAgentStatus({
      kind: "workflowSuccess",
      turn: 1,
      message: "任务执行成功，最终回复已生成",
      yaml: null,
      workflow: {
        planId: "read-clipboard-summary",
        success: true,
        outputNode: "final_report",
        finalOutput: "核心重点总结如下：",
        nodeStates: {
          clip_data: "executed",
          final_report: "executed",
        },
        executedOutputs: {
          clip_data: "一段需要总结的剪贴板内容",
          final_report: "核心重点总结如下：\n\n1. 这是一段剪贴板内容。\n2. 已生成摘要。",
        },
        diagnostics: [
          {
            nodeId: "final_report",
            message: "action timed out after 120000 ms",
          },
        ],
        error: null,
      },
    });
    completeAssistantTurn(
      trace,
      "核心重点总结如下：\n\n1. 这是一段剪贴板内容。\n2. 已生成摘要。",
    );
    trace.details.open = true;
    activeTrace = null;
    setConnection("预览已完成", "success");
    return;
  }

  if (preview !== "confirmation") {
    return;
  }

  const payload: ConfirmationRequest = {
    requestId: "preview-confirmation",
    nodeId: "alarm_730",
    action: "set_alarm",
    inputs: { hour: 7, minutes: 30, message: "Alarm 1", skipUi: true },
    risk: "medium",
    workflowId: "set-alarms-730-to-800",
    items: [
      {
        nodeId: "alarm_730",
        action: "set_alarm",
        inputs: { hour: 7, minutes: 30, message: "Alarm 1", skipUi: true },
        risk: "medium",
      },
      {
        nodeId: "alarm_740",
        action: "set_alarm",
        inputs: { hour: 7, minutes: 40, message: "Alarm 2", skipUi: true },
        risk: "medium",
      },
      {
        nodeId: "alarm_750",
        action: "set_alarm",
        inputs: { hour: 7, minutes: 50, message: "Alarm 3", skipUi: true },
        risk: "medium",
      },
      {
        nodeId: "alarm_800",
        action: "set_alarm",
        inputs: { hour: 8, minutes: 0, message: "Alarm 4", skipUi: true },
        risk: "medium",
      },
    ],
  };

  renderConfirmationRequest(payload);
  showOverlay(confirmationOverlay);
  setConnection("预览确认弹窗", "waiting");
}

function buildConfirmationSummary(payload: ConfirmationRequest, items: ConfirmationItem[]) {
  const action = dominantAction(items);
  const count = items.length;
  const subject = confirmationSubject(action, count);
  return {
    title: `允许${subject}？`,
    meta: `${count} 个操作 · ${riskLabel(payload.risk)}`,
    headline: action === "multiple" ? "应用将执行一组设备操作" : friendlyActionName(action),
    description: actionImpact(action, count, payload.risk),
  };
}

function createSummaryText(headline: string, description: string) {
  const fragment = document.createDocumentFragment();
  const title = document.createElement("strong");
  const copy = document.createElement("p");
  title.textContent = headline;
  copy.textContent = description;
  fragment.append(title, copy);
  return fragment;
}

function createConfirmationItem(item: ConfirmationItem, index: number, batch: boolean) {
  const row = document.createElement("article");
  row.className = `confirmation-item ${item.risk}`;

  const count = document.createElement("span");
  count.className = "confirmation-item-index";
  count.textContent = String(index + 1);

  const content = document.createElement("div");
  content.className = "confirmation-item-content";

  const title = document.createElement("strong");
  title.textContent = confirmationItemTitle(item, index, batch);

  const meta = document.createElement("small");
  meta.textContent = `${friendlyActionName(item.action)} · ${riskLabel(item.risk)}`;

  const chips = document.createElement("div");
  chips.className = "confirmation-item-chips";
  for (const label of confirmationItemHighlights(item)) {
    const chip = document.createElement("span");
    chip.textContent = label;
    chips.append(chip);
  }

  content.append(title, meta);
  if (chips.childElementCount > 0) {
    content.append(chips);
  }
  row.append(count, content);
  return row;
}

function isBatchConfirmation(payload: ConfirmationRequest) {
  return Boolean(payload.workflowId) || (payload.items?.length ?? 0) > 1;
}

function confirmationItems(payload: ConfirmationRequest): ConfirmationItem[] {
  if (payload.items && payload.items.length > 0) {
    return payload.items;
  }
  return [
    {
      nodeId: payload.nodeId,
      action: payload.action,
      inputs: payload.inputs,
      risk: payload.risk,
    },
  ];
}

function dominantAction(items: ConfirmationItem[]) {
  const [first] = items;
  if (!first) {
    return "multiple";
  }
  return items.every((item) => item.action === first.action) ? first.action : "multiple";
}

function confirmationSubject(action: string, count: number) {
  if (action === "set_alarm") {
    return count > 1 ? `设置 ${count} 个闹钟` : "设置闹钟";
  }
  if (action === "multiple") {
    return `执行 ${count} 个设备操作`;
  }
  return friendlyActionName(action);
}

function friendlyActionName(action: string) {
  return (
    {
      set_alarm: "设置闹钟",
      get_device_status: "读取设备状态",
      get_current_media: "查看正在播放的媒体",
      read_clipboard: "读取剪贴板",
      get_clipboard: "读取剪贴板",
      write_clipboard: "写入剪贴板",
      clipboard_read: "读取剪贴板",
      open_url: "打开链接",
      send_notification: "发送通知",
      subagent: "生成总结",
    }[action] ?? action.replace(/[_-]+/g, " ")
  );
}

function actionImpact(action: string, count: number, risk: ConfirmationRisk) {
  if (action === "set_alarm") {
    return `将在设备上创建${count > 1 ? ` ${count} 个` : ""}闹钟。请确认时间和名称无误。`;
  }
  if (action.includes("clipboard")) {
    return "将访问剪贴板内容来完成当前请求，请确认其中没有不想共享的信息。";
  }
  if (action.startsWith("get") || action.startsWith("read")) {
    return "将读取设备上的相关信息，仅用于完成这次对话请求。";
  }
  if (risk === "high" || risk === "critical") {
    return "该操作可能明显改变设备状态，请确认这些变更符合你的意图。";
  }
  return "将代表你在设备上执行操作，请确认后继续。";
}

function confirmationItemTitle(item: ConfirmationItem, index: number, batch: boolean) {
  if (item.action === "set_alarm") {
    const input = asInputRecord(item.inputs);
    const time = formatAlarmTime(input);
    const message = stringValue(input?.message);
    if (time && message) {
      return `${time} · ${message}`;
    }
    if (time) {
      return `${time} 的闹钟`;
    }
  }
  const action = friendlyActionName(item.action);
  return batch ? `${action} ${index + 1}` : action;
}

function confirmationItemHighlights(item: ConfirmationItem) {
  const input = asInputRecord(item.inputs);
  if (!input) {
    return [];
  }

  if (item.action === "set_alarm") {
    return [
      formatAlarmTime(input) ? `时间 ${formatAlarmTime(input)}` : null,
      stringValue(input.message) ? `名称 ${stringValue(input.message)}` : null,
    ].filter(Boolean) as string[];
  }

  return Object.entries(input)
    .slice(0, 3)
    .map(([key, value]) => `${friendlyInputKey(key)} ${formatInlineValue(value)}`);
}

function formatAlarmTime(input: Record<string, unknown> | null) {
  const hour = numberValue(input?.hour);
  const minutes = numberValue(input?.minutes);
  if (hour == null || minutes == null) {
    return null;
  }
  return `${String(hour).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

function friendlyInputKey(key: string) {
  return (
    {
      hour: "小时",
      minutes: "分钟",
      message: "名称",
      url: "链接",
      text: "文本",
      content: "内容",
    }[key] ?? key.replace(/[_-]+/g, " ")
  );
}

function formatInlineValue(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}

function asInputRecord(value: unknown) {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringValue(value: unknown) {
  return typeof value === "string" && value.trim() ? value : null;
}

function numberValue(value: unknown) {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatTechnicalConfirmation(payload: ConfirmationRequest, items: ConfirmationItem[]) {
  return JSON.stringify(
    {
      requestId: payload.requestId,
      nodeId: payload.nodeId,
      action: payload.action,
      inputs: payload.inputs,
      risk: payload.risk,
      workflowId: payload.workflowId ?? null,
      items,
    },
    null,
    2,
  );
}

function riskIcon(risk: ConfirmationRisk) {
  return risk === "low" ? "i" : "!";
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
    setConnection("正在启动任务", "running");
  }
}

function setConnection(label: string, state: string) {
  connectionLabel.textContent = label;
  connectionLabel.parentElement!.className = `connection-pill ${state}`;
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

use serde::{Deserialize, Serialize};

/// Action 的风险等级
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// 只读/纯计算操作
    Low,
    /// 本地写入操作
    Medium,
    /// 外部通信（网络、跨进程）
    High,
    /// 不可逆操作
    Critical,
}

/// 每个 Action/Node 的执行策略
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionPolicy {
    /// 超时（毫秒）
    pub timeout_ms: u64,
    /// 最大重试次数
    pub max_retries: u32,
    /// 是否需要用户确认（预留）
    pub requires_confirmation: bool,
    /// 是否收集系统证据（预留）
    pub collect_evidence: bool,
    /// 风险等级
    pub risk_level: RiskLevel,
}

impl Default for ActionPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            max_retries: 0,
            requires_confirmation: false,
            collect_evidence: false,
            risk_level: RiskLevel::Low,
        }
    }
}

impl ActionPolicy {
    pub fn with_risk(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn with_confirmation(mut self, require: bool) -> Self {
        self.requires_confirmation = require;
        self
    }

    pub fn with_evidence(mut self, collect: bool) -> Self {
        self.collect_evidence = collect;
        self
    }

    /// 是否需要串行执行（高风险或不可逆）
    pub fn requires_serial(&self) -> bool {
        matches!(self.risk_level, RiskLevel::Critical | RiskLevel::High)
    }

    /// 是否允许执行（需要确认但无 UI 则跳过）
    pub fn can_execute(&self) -> bool {
        !self.requires_confirmation
    }
}

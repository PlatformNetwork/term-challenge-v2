use crate::rpc::RpcClient;
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Leaderboard,
    Evaluation,
    Submission,
    Network,
}

impl Tab {
    pub const ALL: [Tab; 4] = [
        Tab::Leaderboard,
        Tab::Evaluation,
        Tab::Submission,
        Tab::Network,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Leaderboard => "Leaderboard",
            Tab::Evaluation => "Evaluation",
            Tab::Submission => "Submission",
            Tab::Network => "Network",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Tab::Leaderboard => 0,
            Tab::Evaluation => 1,
            Tab::Submission => 2,
            Tab::Network => 3,
        }
    }
}

pub struct LeaderboardRow {
    pub rank: u32,
    pub miner_hotkey: String,
    pub score: f64,
    pub pass_rate: f64,
    pub submissions: u32,
    pub last_submission: String,
}

pub struct EvalTaskRow {
    pub task_id: String,
    pub status: String,
    pub score: f64,
    pub duration_ms: u64,
    pub error: Option<String>,
}

pub struct NetworkStatus {
    pub epoch: u64,
    pub phase: String,
    pub block_height: u64,
    pub validators: usize,
    pub connected: bool,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            epoch: 0,
            phase: "unknown".to_string(),
            block_height: 0,
            validators: 0,
            connected: false,
        }
    }
}

pub struct App {
    pub tab: Tab,
    pub rpc_url: String,
    pub hotkey: Option<String>,
    pub challenge_id: Option<String>,
    pub leaderboard: Vec<LeaderboardRow>,
    pub evaluation_progress: Vec<EvalTaskRow>,
    pub network_status: NetworkStatus,
    pub scroll_offset: usize,
    pub last_refresh: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new(rpc_url: String, hotkey: Option<String>, challenge_id: Option<String>) -> Self {
        Self {
            tab: Tab::Leaderboard,
            rpc_url,
            hotkey,
            challenge_id,
            leaderboard: Vec::new(),
            evaluation_progress: Vec::new(),
            network_status: NetworkStatus::default(),
            scroll_offset: 0,
            last_refresh: None,
            error_message: None,
            should_quit: false,
        }
    }

    pub fn set_tab_from_str(&mut self, s: &str) {
        self.tab = match s.to_lowercase().as_str() {
            "leaderboard" => Tab::Leaderboard,
            "evaluation" => Tab::Evaluation,
            "submission" => Tab::Submission,
            "network" => Tab::Network,
            _ => Tab::Leaderboard,
        };
        self.scroll_offset = 0;
    }

    pub fn next_tab(&mut self) {
        let idx = self.tab.index();
        let next = (idx + 1) % Tab::ALL.len();
        self.tab = Tab::ALL[next];
        self.scroll_offset = 0;
    }

    pub fn prev_tab(&mut self) {
        let idx = self.tab.index();
        let prev = if idx == 0 {
            Tab::ALL.len() - 1
        } else {
            idx - 1
        };
        self.tab = Tab::ALL[prev];
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub async fn refresh(&mut self, rpc: &RpcClient) {
        self.error_message = None;

        if let Err(e) = self.refresh_network(rpc).await {
            self.error_message = Some(format!("Network: {e}"));
            self.network_status.connected = false;
            return;
        }
        self.network_status.connected = true;

        if self.challenge_id.is_none() {
            match rpc.fetch_challenge_list().await {
                Ok(challenges) if challenges.len() == 1 => {
                    self.challenge_id = Some(challenges[0].id.clone());
                }
                Ok(_) => {}
                Err(e) => {
                    self.error_message = Some(format!("Challenges: {e}"));
                }
            }
        }

        if let Some(cid) = &self.challenge_id {
            let cid = cid.clone();
            match rpc.fetch_leaderboard(&cid).await {
                Ok(rows) => self.leaderboard = rows,
                Err(e) => {
                    self.error_message = Some(format!("Leaderboard: {e}"));
                }
            }
        }

        if let Some(hotkey) = &self.hotkey {
            let hotkey = hotkey.clone();
            match rpc.fetch_evaluation_progress(&hotkey).await {
                Ok(tasks) => self.evaluation_progress = tasks,
                Err(e) => {
                    tracing::debug!("Evaluation progress: {e}");
                }
            }
        }

        self.last_refresh = Some(Utc::now());
    }

    async fn refresh_network(&mut self, rpc: &RpcClient) -> anyhow::Result<()> {
        let _ = rpc.fetch_system_health().await?;

        let epoch_info = rpc.fetch_epoch_info().await?;
        self.network_status.epoch = epoch_info.epoch;
        self.network_status.phase = epoch_info.phase;
        self.network_status.block_height = epoch_info.block_height;

        match rpc.fetch_validator_count().await {
            Ok(count) => self.network_status.validators = count,
            Err(e) => {
                tracing::warn!("Failed to fetch validator count: {e}");
            }
        }

        Ok(())
    }
}

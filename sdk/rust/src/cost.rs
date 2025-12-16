//! Cost tracking for LLM usage

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Model pricing in USD per million tokens (input, output)
pub struct ModelPricing;

impl ModelPricing {
    pub fn get(model: &str) -> (f64, f64) {
        match model.to_lowercase().as_str() {
            "openai/gpt-4o" => (2.5, 10.0),
            "openai/gpt-4o-mini" | "gpt-4o-mini" => (0.15, 0.6),
            "openai/gpt-4-turbo" => (10.0, 30.0),
            "openai/o1-preview" => (15.0, 60.0),
            "openai/o1-mini" => (3.0, 12.0),
            "anthropic/claude-3.5-sonnet" | "claude-3.5-sonnet" => (3.0, 15.0),
            "anthropic/claude-3-haiku" | "claude-3-haiku" => (0.25, 1.25),
            "anthropic/claude-3-opus" => (15.0, 75.0),
            "meta-llama/llama-3.1-70b-instruct" => (0.52, 0.75),
            "mistralai/mixtral-8x7b-instruct" => (0.24, 0.24),
            "google/gemini-pro" => (0.125, 0.375),
            "deepseek/deepseek-chat" => (0.14, 0.28),
            _ => (0.15, 0.6), // Default to gpt-4o-mini pricing
        }
    }

    pub fn calculate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        let (input_price, output_price) = Self::get(model);
        (input_tokens as f64 * input_price / 1_000_000.0)
            + (output_tokens as f64 * output_price / 1_000_000.0)
    }
}

/// Record of a single LLM call
#[derive(Clone, Debug)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost: f64,
}

impl UsageRecord {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Tracks LLM usage and costs
#[derive(Clone)]
pub struct CostTracker {
    inner: Arc<Mutex<CostTrackerInner>>,
}

struct CostTrackerInner {
    limit: f64,
    records: Vec<UsageRecord>,
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new(10.0)
    }
}

impl CostTracker {
    /// Create a new cost tracker with the given limit
    pub fn new(limit: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CostTrackerInner {
                limit,
                records: Vec::new(),
            })),
        }
    }

    /// Get the cost limit
    pub fn limit(&self) -> f64 {
        self.inner.lock().unwrap().limit
    }

    /// Set the cost limit
    pub fn set_limit(&self, limit: f64) {
        self.inner.lock().unwrap().limit = limit;
    }

    /// Get total cost incurred
    pub fn total_cost(&self) -> f64 {
        self.inner
            .lock()
            .unwrap()
            .records
            .iter()
            .map(|r| r.cost)
            .sum()
    }

    /// Get remaining budget
    pub fn remaining(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        let total: f64 = inner.records.iter().map(|r| r.cost).sum();
        (inner.limit - total).max(0.0)
    }

    /// Get total tokens used
    pub fn total_tokens(&self) -> u64 {
        self.inner
            .lock()
            .unwrap()
            .records
            .iter()
            .map(|r| r.total_tokens())
            .sum()
    }

    /// Record a usage
    pub fn add(&self, model: &str, input_tokens: u64, output_tokens: u64, cost: f64) -> UsageRecord {
        let record = UsageRecord {
            timestamp: Utc::now(),
            model: model.to_string(),
            input_tokens,
            output_tokens,
            cost,
        };

        self.inner.lock().unwrap().records.push(record.clone());
        record
    }

    /// Check if an estimated cost can be afforded
    pub fn can_afford(&self, estimated_cost: f64) -> bool {
        self.remaining() >= estimated_cost
    }

    /// Estimate cost for a request
    pub fn estimate_cost(&self, model: &str, input_tokens: u64, max_output_tokens: u64) -> f64 {
        ModelPricing::calculate_cost(model, input_tokens, max_output_tokens)
    }

    /// Get usage summary by model
    pub fn summary(&self) -> HashMap<String, (f64, u64)> {
        let inner = self.inner.lock().unwrap();
        let mut summary: HashMap<String, (f64, u64)> = HashMap::new();

        for record in &inner.records {
            let entry = summary.entry(record.model.clone()).or_insert((0.0, 0));
            entry.0 += record.cost;
            entry.1 += record.total_tokens();
        }

        summary
    }

    /// Reset all records
    pub fn reset(&self) {
        self.inner.lock().unwrap().records.clear();
    }
}

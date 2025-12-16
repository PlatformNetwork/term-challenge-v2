//! Scoring system for terminal benchmark

use crate::task::{Difficulty, Task, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Score calculator for terminal benchmark
pub struct ScoreCalculator {
    /// Weight for each difficulty level
    difficulty_weights: HashMap<Difficulty, f64>,
    /// Time bonus factor (faster completion = higher score)
    time_bonus_factor: f64,
    /// Maximum time bonus multiplier
    max_time_bonus: f64,
}

impl Default for ScoreCalculator {
    fn default() -> Self {
        let mut difficulty_weights = HashMap::new();
        difficulty_weights.insert(Difficulty::Easy, 1.0);
        difficulty_weights.insert(Difficulty::Medium, 2.0);
        difficulty_weights.insert(Difficulty::Hard, 3.0);

        Self {
            difficulty_weights,
            time_bonus_factor: 0.001, // 0.1% bonus per second saved
            max_time_bonus: 1.5,      // Max 50% bonus
        }
    }
}

impl ScoreCalculator {
    /// Create a new score calculator with custom weights
    pub fn new(difficulty_weights: HashMap<Difficulty, f64>) -> Self {
        Self {
            difficulty_weights,
            ..Default::default()
        }
    }

    /// Calculate score for a single task result
    pub fn score_task(&self, task: &Task, result: &TaskResult) -> f64 {
        if !result.passed {
            return 0.0;
        }

        // Base score from difficulty
        let base_weight = self
            .difficulty_weights
            .get(&task.config.difficulty)
            .copied()
            .unwrap_or(1.0);

        // Time bonus: faster completion gets bonus
        let timeout_ms = task.config.timeout_secs as u64 * 1000;
        let time_saved_ms = timeout_ms.saturating_sub(result.execution_time_ms);
        let time_bonus = 1.0
            + (time_saved_ms as f64 * self.time_bonus_factor / 1000.0)
                .min(self.max_time_bonus - 1.0);

        base_weight * time_bonus
    }

    /// Calculate aggregate score for multiple task results
    pub fn calculate_aggregate(&self, tasks: &[&Task], results: &[TaskResult]) -> AggregateScore {
        let mut total_score = 0.0;
        let mut max_possible = 0.0;
        let mut passed = 0;
        let mut failed = 0;
        let mut by_difficulty: HashMap<Difficulty, DifficultyStats> = HashMap::new();

        for (task, result) in tasks.iter().zip(results.iter()) {
            let score = self.score_task(task, result);
            let max_score = self
                .difficulty_weights
                .get(&task.config.difficulty)
                .copied()
                .unwrap_or(1.0)
                * self.max_time_bonus;

            total_score += score;
            max_possible += max_score;

            if result.passed {
                passed += 1;
            } else {
                failed += 1;
            }

            // Track by difficulty
            let stats = by_difficulty.entry(task.config.difficulty).or_default();
            stats.total += 1;
            if result.passed {
                stats.passed += 1;
            }
            stats.total_score += score;
        }

        let normalized_score = if max_possible > 0.0 {
            total_score / max_possible
        } else {
            0.0
        };

        AggregateScore {
            total_score,
            normalized_score,
            max_possible,
            tasks_passed: passed,
            tasks_failed: failed,
            pass_rate: if passed + failed > 0 {
                passed as f64 / (passed + failed) as f64
            } else {
                0.0
            },
            by_difficulty,
        }
    }

    /// Convert aggregate score to weight assignment (0.0 - 1.0)
    pub fn to_weight(&self, score: &AggregateScore) -> f64 {
        // Use normalized score as weight
        score.normalized_score.clamp(0.0, 1.0)
    }
}

/// Statistics for a difficulty level
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DifficultyStats {
    pub total: usize,
    pub passed: usize,
    pub total_score: f64,
}

impl DifficultyStats {
    pub fn pass_rate(&self) -> f64 {
        if self.total > 0 {
            self.passed as f64 / self.total as f64
        } else {
            0.0
        }
    }
}

/// Aggregate score for an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregateScore {
    /// Total raw score
    pub total_score: f64,
    /// Normalized score (0.0 - 1.0)
    pub normalized_score: f64,
    /// Maximum possible score
    pub max_possible: f64,
    /// Number of tasks passed
    pub tasks_passed: usize,
    /// Number of tasks failed
    pub tasks_failed: usize,
    /// Pass rate (0.0 - 1.0)
    pub pass_rate: f64,
    /// Breakdown by difficulty
    pub by_difficulty: HashMap<Difficulty, DifficultyStats>,
}

impl AggregateScore {
    /// Get total tasks
    pub fn total_tasks(&self) -> usize {
        self.tasks_passed + self.tasks_failed
    }

    /// Get percentage score
    pub fn percentage(&self) -> f64 {
        self.normalized_score * 100.0
    }
}

/// Leaderboard entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub agent_hash: String,
    pub score: AggregateScore,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

/// Leaderboard for tracking agent performance
pub struct Leaderboard {
    entries: Vec<LeaderboardEntry>,
    max_entries: usize,
}

impl Leaderboard {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add or update an entry
    pub fn update(&mut self, agent_hash: String, score: AggregateScore) {
        // Remove existing entry for this agent
        self.entries.retain(|e| e.agent_hash != agent_hash);

        // Add new entry
        self.entries.push(LeaderboardEntry {
            agent_hash,
            score,
            evaluated_at: chrono::Utc::now(),
        });

        // Sort by normalized score (descending)
        self.entries.sort_by(|a, b| {
            b.score
                .normalized_score
                .partial_cmp(&a.score.normalized_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Trim to max entries
        self.entries.truncate(self.max_entries);
    }

    /// Get top N entries
    pub fn top(&self, n: usize) -> &[LeaderboardEntry] {
        &self.entries[..n.min(self.entries.len())]
    }

    /// Get rank for an agent
    pub fn rank(&self, agent_hash: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.agent_hash == agent_hash)
            .map(|i| i + 1)
    }

    /// Get entry for an agent
    pub fn get(&self, agent_hash: &str) -> Option<&LeaderboardEntry> {
        self.entries.iter().find(|e| e.agent_hash == agent_hash)
    }

    /// Get all entries
    pub fn all(&self) -> &[LeaderboardEntry] {
        &self.entries
    }
}

impl Default for Leaderboard {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskConfig;

    fn create_test_task(difficulty: Difficulty) -> Task {
        Task::from_components(
            "test".to_string(),
            TaskConfig {
                name: "Test Task".to_string(),
                instruction: "Test".to_string(),
                difficulty,
                timeout_secs: 180.0,
                ..Default::default()
            },
            "#!/bin/bash\nexit 0".to_string(),
            None,
            None,
        )
    }

    #[test]
    fn test_score_passed_task() {
        let calculator = ScoreCalculator::default();
        let task = create_test_task(Difficulty::Medium);
        let result = TaskResult::success(
            "test".to_string(),
            "agent1".to_string(),
            60000, // 60 seconds
            String::new(),
            String::new(),
        );

        let score = calculator.score_task(&task, &result);
        assert!(score > 0.0);
        assert!(score >= 2.0); // At least base difficulty weight
    }

    #[test]
    fn test_score_failed_task() {
        let calculator = ScoreCalculator::default();
        let task = create_test_task(Difficulty::Easy);
        let result = TaskResult::failure(
            "test".to_string(),
            "agent1".to_string(),
            60000,
            String::new(),
            String::new(),
            "Test failed".to_string(),
        );

        let score = calculator.score_task(&task, &result);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_aggregate_score() {
        let calculator = ScoreCalculator::default();

        let task1 = create_test_task(Difficulty::Easy);
        let task2 = create_test_task(Difficulty::Hard);

        let result1 = TaskResult::success(
            "t1".to_string(),
            "a".to_string(),
            60000,
            String::new(),
            String::new(),
        );
        let result2 = TaskResult::failure(
            "t2".to_string(),
            "a".to_string(),
            60000,
            String::new(),
            String::new(),
            "fail".to_string(),
        );

        let aggregate = calculator.calculate_aggregate(&[&task1, &task2], &[result1, result2]);

        assert_eq!(aggregate.tasks_passed, 1);
        assert_eq!(aggregate.tasks_failed, 1);
        assert_eq!(aggregate.pass_rate, 0.5);
    }

    #[test]
    fn test_leaderboard() {
        let mut leaderboard = Leaderboard::new(10);

        let score1 = AggregateScore {
            total_score: 10.0,
            normalized_score: 0.8,
            max_possible: 12.5,
            tasks_passed: 8,
            tasks_failed: 2,
            pass_rate: 0.8,
            by_difficulty: HashMap::new(),
        };

        let score2 = AggregateScore {
            total_score: 12.0,
            normalized_score: 0.95,
            max_possible: 12.5,
            tasks_passed: 10,
            tasks_failed: 0,
            pass_rate: 1.0,
            by_difficulty: HashMap::new(),
        };

        leaderboard.update("agent1".to_string(), score1);
        leaderboard.update("agent2".to_string(), score2);

        assert_eq!(leaderboard.rank("agent2"), Some(1));
        assert_eq!(leaderboard.rank("agent1"), Some(2));
    }
}

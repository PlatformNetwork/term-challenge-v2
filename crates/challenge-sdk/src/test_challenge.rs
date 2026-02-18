//! Simple test challenge for integration testing
//!
//! This module provides a simple challenge implementation using the new
//! ServerChallenge API for testing purposes.

use crate::{
    error::ChallengeError,
    server::{
        EvaluationRequest, EvaluationResponse, ServerChallenge, ValidationRequest,
        ValidationResponse,
    },
    types::ChallengeId,
};
use async_trait::async_trait;
use serde_json::{json, Value};

/// Simple test challenge that returns scores based on submission data
pub struct SimpleTestChallenge {
    id: String,
    name: String,
    version: String,
}

impl SimpleTestChallenge {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: "simple-test-challenge".to_string(),
            name: name.into(),
            version: "0.1.0".to_string(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
}

impl Default for SimpleTestChallenge {
    fn default() -> Self {
        Self::new("Simple Test Challenge")
    }
}

#[async_trait]
impl ServerChallenge for SimpleTestChallenge {
    fn challenge_id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    async fn evaluate(&self, req: EvaluationRequest) -> Result<EvaluationResponse, ChallengeError> {
        // Simple scoring based on data content
        let base_score = 0.5;

        // Add bonus based on payload
        let payload_bonus = if let Some(bonus) = req.data.get("bonus").and_then(|v| v.as_f64()) {
            bonus.clamp(0.0, 0.5)
        } else {
            0.0
        };

        let score = (base_score + payload_bonus).clamp(0.0, 1.0);

        Ok(EvaluationResponse::success(
            &req.request_id,
            score,
            json!({
                "base_score": base_score,
                "bonus": payload_bonus,
                "participant": req.participant_id
            }),
        ))
    }

    async fn validate(&self, req: ValidationRequest) -> Result<ValidationResponse, ChallengeError> {
        // Simple validation - just check if data is not empty
        let is_valid = !req.data.is_null() && req.data != json!({});

        if is_valid {
            Ok(ValidationResponse {
                valid: true,
                errors: vec![],
                warnings: vec![],
            })
        } else {
            Ok(ValidationResponse {
                valid: false,
                errors: vec!["Empty or null data".to_string()],
                warnings: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_challenge_evaluate() {
        let challenge = SimpleTestChallenge::default();

        let req = EvaluationRequest {
            request_id: "test-123".to_string(),
            submission_id: "sub-123".to_string(),
            participant_id: "participant-1".to_string(),
            data: json!({"bonus": 0.2}),
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert!(result.score >= 0.5);
        assert!(result.score <= 1.0);
    }

    #[tokio::test]
    async fn test_simple_challenge_validate() {
        let challenge = SimpleTestChallenge::default();

        // Valid request
        let req = ValidationRequest {
            data: json!({"some": "data"}),
        };

        let result = challenge.validate(req).await.unwrap();
        assert!(result.valid);

        // Invalid request (empty data)
        let req = ValidationRequest { data: json!({}) };

        let result = challenge.validate(req).await.unwrap();
        assert!(!result.valid);
    }

    #[test]
    fn test_simple_challenge_with_id() {
        let challenge = SimpleTestChallenge::new("Test").with_id("custom-id");

        assert_eq!(challenge.challenge_id(), "custom-id");
    }

    #[test]
    fn test_simple_challenge_challenge_id() {
        let challenge = SimpleTestChallenge::default();
        assert_eq!(challenge.challenge_id(), "simple-test-challenge");
    }

    #[test]
    fn test_simple_challenge_name() {
        let challenge = SimpleTestChallenge::new("My Test Challenge");
        assert_eq!(challenge.name(), "My Test Challenge");
    }

    #[test]
    fn test_simple_challenge_version() {
        let challenge = SimpleTestChallenge::default();
        assert_eq!(challenge.version(), "0.1.0");
    }

    #[tokio::test]
    async fn test_evaluate_with_zero_bonus() {
        let challenge = SimpleTestChallenge::default();

        let req = EvaluationRequest {
            request_id: "test-456".to_string(),
            submission_id: "sub-456".to_string(),
            participant_id: "participant-2".to_string(),
            data: json!({"bonus": 0.0}),
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert_eq!(result.score, 0.5); // base score only
    }

    #[tokio::test]
    async fn test_evaluate_with_max_bonus() {
        let challenge = SimpleTestChallenge::default();

        let req = EvaluationRequest {
            request_id: "test-789".to_string(),
            submission_id: "sub-789".to_string(),
            participant_id: "participant-3".to_string(),
            data: json!({"bonus": 0.5}),
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert_eq!(result.score, 1.0); // base + max bonus
    }

    #[tokio::test]
    async fn test_evaluate_with_excessive_bonus() {
        let challenge = SimpleTestChallenge::default();

        let req = EvaluationRequest {
            request_id: "test-999".to_string(),
            submission_id: "sub-999".to_string(),
            participant_id: "participant-4".to_string(),
            data: json!({"bonus": 1.0}), // More than max 0.5
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert_eq!(result.score, 1.0); // Clamped to 1.0
    }

    #[tokio::test]
    async fn test_evaluate_without_bonus() {
        let challenge = SimpleTestChallenge::default();

        let req = EvaluationRequest {
            request_id: "test-000".to_string(),
            submission_id: "sub-000".to_string(),
            participant_id: "participant-5".to_string(),
            data: json!({"other_field": "value"}),
            metadata: None,
            epoch: 1,
            deadline: None,
        };

        let result = challenge.evaluate(req).await.unwrap();

        assert!(result.success);
        assert_eq!(result.score, 0.5); // base score only, no bonus field
    }

    #[tokio::test]
    async fn test_validate_with_null_data() {
        let challenge = SimpleTestChallenge::default();

        let req = ValidationRequest { data: json!(null) };

        let result = challenge.validate(req).await.unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }
}

use platform_challenge_sdk::ChallengeDatabase;

use crate::types::TaskDefinition;

pub fn store_active_dataset(
    db: &ChallengeDatabase,
    tasks: &[TaskDefinition],
) -> Result<(), platform_challenge_sdk::ChallengeError> {
    db.kv_set("active_dataset", &tasks.to_vec())
}

pub fn get_active_dataset(db: &ChallengeDatabase) -> Vec<TaskDefinition> {
    db.kv_get::<Vec<TaskDefinition>>("active_dataset")
        .ok()
        .flatten()
        .unwrap_or_default()
}

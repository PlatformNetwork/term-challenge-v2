use platform_challenge_sdk::ChallengeDatabase;

use crate::types::SubmissionRecord;

pub fn register_submission_name(
    db: &ChallengeDatabase,
    name: &str,
    hotkey: &str,
    epoch: u64,
    package_hash: &str,
) -> Result<bool, platform_challenge_sdk::ChallengeError> {
    let key = format!("submission_name:{}", name);

    let existing = db.kv_get::<SubmissionRecord>(&key).ok().flatten();

    let version = match &existing {
        Some(record) => {
            if record.hotkey != hotkey {
                return Ok(false);
            }
            record.version + 1
        }
        None => 1,
    };

    let record = SubmissionRecord {
        name: name.to_string(),
        hotkey: hotkey.to_string(),
        epoch,
        version,
        package_hash: package_hash.to_string(),
    };

    db.kv_set(&key, &record)?;
    Ok(true)
}

pub fn get_submission_by_name(db: &ChallengeDatabase, name: &str) -> Option<SubmissionRecord> {
    let key = format!("submission_name:{}", name);
    db.kv_get::<SubmissionRecord>(&key).ok().flatten()
}

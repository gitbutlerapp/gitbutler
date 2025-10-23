use but_secret::{Sensitive, secret};
use tracing::{debug, info, warn};

/// Known secret handles that were previously stored in Global namespace
/// and need to be migrated to BuildKind namespace
const KNOWN_GLOBAL_SECRET_HANDLES: &[&str] = &[
    "aiOpenAIKey",
    "aiAnthropicKey",
    "TokenMemoryService-authToken",
];

/// Migrate secrets from Global namespace to BuildKind namespace.
/// This function should be called once on application startup.
/// It will only migrate secrets that exist in Global namespace and don't
/// already exist in BuildKind namespace.
pub fn migrate_global_secrets_to_build_kind() {
    info!("Starting migration of global secrets to build-kind namespace");
    
    let mut migrated_count = 0;
    let mut skipped_count = 0;
    let mut error_count = 0;
    
    for handle in KNOWN_GLOBAL_SECRET_HANDLES {
        match migrate_single_secret(handle) {
            MigrationResult::Migrated => {
                migrated_count += 1;
                info!("Migrated secret: {}", handle);
            }
            MigrationResult::Skipped => {
                skipped_count += 1;
                debug!("Skipped migration for secret: {} (not found in global or already exists in build-kind)", handle);
            }
            MigrationResult::Error(err) => {
                error_count += 1;
                warn!("Failed to migrate secret {}: {}", handle, err);
            }
        }
    }
    
    info!(
        "Secret migration complete: {} migrated, {} skipped, {} errors",
        migrated_count, skipped_count, error_count
    );
}

enum MigrationResult {
    Migrated,
    Skipped,
    Error(String),
}

fn migrate_single_secret(handle: &str) -> MigrationResult {
    // Check if secret exists in BuildKind namespace already
    match secret::retrieve(handle, secret::Namespace::BuildKind) {
        Ok(Some(_)) => {
            // Secret already exists in BuildKind, no need to migrate
            return MigrationResult::Skipped;
        }
        Ok(None) => {
            // Secret doesn't exist in BuildKind, continue with migration
        }
        Err(err) => {
            return MigrationResult::Error(format!("Error checking BuildKind namespace: {}", err));
        }
    }
    
    // Try to retrieve from Global namespace
    let global_secret = match secret::retrieve(handle, secret::Namespace::Global) {
        Ok(Some(secret)) => secret,
        Ok(None) => {
            // No secret in Global namespace, nothing to migrate
            return MigrationResult::Skipped;
        }
        Err(err) => {
            return MigrationResult::Error(format!("Error retrieving from Global namespace: {}", err));
        }
    };
    
    // Persist to BuildKind namespace
    if let Err(err) = secret::persist(handle, &global_secret, secret::Namespace::BuildKind) {
        return MigrationResult::Error(format!("Error persisting to BuildKind namespace: {}", err));
    }
    
    // Optionally delete from Global namespace after successful migration
    // We'll keep the global secret for now for safety, in case the user needs to rollback
    // if let Err(err) = secret::delete(handle, secret::Namespace::Global) {
    //     warn!("Failed to delete global secret after migration: {}", err);
    // }
    
    MigrationResult::Migrated
}

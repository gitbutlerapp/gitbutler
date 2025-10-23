use but_secret::secret;
use tracing::{debug, info, warn};

#[cfg(test)]
use but_secret::Sensitive;

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

impl std::fmt::Debug for MigrationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationResult::Migrated => write!(f, "Migrated"),
            MigrationResult::Skipped => write!(f, "Skipped"),
            MigrationResult::Error(err) => write!(f, "Error({})", err),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a working keyring/keychain system.
    // They may be skipped in CI environments where DBus is not available.
    // The underlying secret storage functionality is tested in the but-secret crate.

    #[test]
    #[ignore = "Requires system keyring access"]
    fn test_migrate_single_secret_no_global_secret() {
        // Set up a unique namespace for this test
        secret::set_application_namespace("test-migrate-no-global");
        
        let handle = "test-secret-no-global";
        
        // Ensure neither namespace has this secret
        let _ = secret::delete(handle, secret::Namespace::Global);
        let _ = secret::delete(handle, secret::Namespace::BuildKind);
        
        // Should skip if no global secret exists
        match migrate_single_secret(handle) {
            MigrationResult::Skipped => {
                // Success - this is expected
            }
            other => panic!("Expected Skipped but got {:?}", other),
        }
    }

    #[test]
    #[ignore = "Requires system keyring access"]
    fn test_migrate_single_secret_success() {
        // Set up a unique namespace for this test
        secret::set_application_namespace("test-migrate-success");
        
        let handle = "test-secret-migrate";
        let secret_value = Sensitive("test-value-123".to_string());
        
        // Clean up any existing secrets
        let _ = secret::delete(handle, secret::Namespace::Global);
        let _ = secret::delete(handle, secret::Namespace::BuildKind);
        
        // Set up a global secret
        secret::persist(handle, &secret_value, secret::Namespace::Global)
            .expect("Failed to persist test secret to Global namespace");
        
        // Migrate the secret
        match migrate_single_secret(handle) {
            MigrationResult::Migrated => {
                // Success - verify the secret was migrated
                let migrated = secret::retrieve(handle, secret::Namespace::BuildKind)
                    .expect("Failed to retrieve migrated secret")
                    .expect("Migrated secret not found");
                assert_eq!(migrated.0, "test-value-123");
            }
            other => panic!("Expected Migrated but got {:?}", other),
        }
        
        // Clean up
        let _ = secret::delete(handle, secret::Namespace::Global);
        let _ = secret::delete(handle, secret::Namespace::BuildKind);
    }

    #[test]
    #[ignore = "Requires system keyring access"]
    fn test_migrate_single_secret_already_exists_in_buildkind() {
        // Set up a unique namespace for this test
        secret::set_application_namespace("test-migrate-exists");
        
        let handle = "test-secret-exists";
        let global_value = Sensitive("global-value".to_string());
        let buildkind_value = Sensitive("buildkind-value".to_string());
        
        // Clean up
        let _ = secret::delete(handle, secret::Namespace::Global);
        let _ = secret::delete(handle, secret::Namespace::BuildKind);
        
        // Set up secrets in both namespaces
        secret::persist(handle, &global_value, secret::Namespace::Global)
            .expect("Failed to persist test secret to Global namespace");
        secret::persist(handle, &buildkind_value, secret::Namespace::BuildKind)
            .expect("Failed to persist test secret to BuildKind namespace");
        
        // Should skip if BuildKind already has the secret
        match migrate_single_secret(handle) {
            MigrationResult::Skipped => {
                // Success - verify the BuildKind secret wasn't overwritten
                let existing = secret::retrieve(handle, secret::Namespace::BuildKind)
                    .expect("Failed to retrieve existing secret")
                    .expect("Existing secret not found");
                assert_eq!(existing.0, "buildkind-value", "Existing secret was overwritten");
            }
            other => panic!("Expected Skipped but got {:?}", other),
        }
        
        // Clean up
        let _ = secret::delete(handle, secret::Namespace::Global);
        let _ = secret::delete(handle, secret::Namespace::BuildKind);
    }
}

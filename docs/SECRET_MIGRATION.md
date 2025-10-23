# Secret Management Migration

## Overview

This document describes the changes made to retire `secret_set_global()` and migrate to build-kind-specific secret storage.

## Problem

Previously, all secrets were stored in a single Global namespace, which meant that different build configurations (dev, nightly, production, test) shared the same secrets. This could lead to:
- Conflicts between different builds running on the same system
- Accidental data sharing between development and production environments
- Difficulty in testing with isolated configurations

## Solution

### New API Functions

Three new API functions were added that use the BuildKind namespace:

1. **`secret_get(handle: String)`** - Retrieves a secret from BuildKind namespace
2. **`secret_set(handle: String, secret: String)`** - Stores a secret in BuildKind namespace
3. **`secret_delete(handle: String)`** - Deletes a secret from BuildKind namespace

### Deprecated Functions

The old global secret functions are deprecated but kept for backwards compatibility:
- `secret_get_global()` - deprecated, use `secret_get()` instead
- `secret_set_global()` - deprecated, use `secret_set()` instead
- `secret_delete_global()` - deprecated, use `secret_delete()` instead

### Automatic Migration

On Tauri application startup, the following secrets are automatically migrated from Global to BuildKind namespace:
- `aiOpenAIKey` - OpenAI API key for AI features
- `aiAnthropicKey` - Anthropic API key for Claude AI features
- `TokenMemoryService-authToken` - GitButler authentication token

#### Migration Behavior

- **Non-destructive**: Original secrets in Global namespace are preserved
- **Idempotent**: Migration can be run multiple times without issues
- **Smart**: Only migrates if:
  - Secret exists in Global namespace, AND
  - Secret does NOT already exist in BuildKind namespace
- **Logged**: All migration results are logged for debugging

## Build Configurations

Different builds have different identifiers, which creates separate secret namespaces:

| Build Type | Identifier | Use Case |
|------------|-----------|----------|
| Development | `com.gitbutler.app.dev` | Local development |
| Nightly | `com.gitbutler.app.nightly` | Nightly builds |
| Production | `com.gitbutler.app` | Stable releases |
| Test | `com.gitbutler.app.test` | E2E testing |

## Frontend Changes

The `RustSecretService` class in `apps/desktop/src/lib/secrets/secretsService.ts` now uses the new API functions:

```typescript
async get(handle: string) {
    const secret = await this.backend.invoke<string>('secret_get', { handle });
    if (secret) return secret;
}

async set(handle: string, secret: string) {
    await this.backend.invoke('secret_set', { handle, secret });
}

async delete(handle: string) {
    await this.backend.invoke('secret_delete', { handle });
}
```

## Implementation Details

### Secret Storage

Secrets are stored using the system's native secure storage:
- **macOS**: Keychain
- **Windows**: Windows Credential Store
- **Linux**: Secret Service API (libsecret/gnome-keyring)

### Namespace Format

Secrets are stored with a prefix based on the namespace:
- BuildKind: `{identifier}-{handle}` (e.g., `com.gitbutler.app.dev-aiOpenAIKey`)
- Global: `gitbutler-{handle}` (e.g., `gitbutler-aiOpenAIKey`)

## Testing

Tests are included in `crates/gitbutler-tauri/src/secret_migration.rs` but are marked as `#[ignore]` because they require system keyring access. To run them:

```bash
cargo test -p gitbutler-tauri secret_migration -- --ignored
```

The underlying secret storage functionality is tested in the `but-secret` crate.

## Backwards Compatibility

- Old API functions are deprecated but still functional
- Migration happens automatically on first startup
- No user action required
- Original secrets preserved for safety

## Future Work

In a future release, the deprecated global secret functions can be completely removed once all users have migrated.

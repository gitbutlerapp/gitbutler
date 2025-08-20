# Git Repository Trust Error Handling

This document describes the implementation of Git repository trust error handling in GitButler, addressing issue #9913.

## Overview

GitButler now properly handles Git security/trust-related errors when opening repositories. This feature allows users to safely open repositories that Git considers untrusted while being informed of the security implications.

## Problem

Git has security mechanisms that prevent opening repositories in certain locations (e.g., repositories owned by different users, on network drives, or in shared folders). These security checks can cause GitButler to fail when trying to open otherwise valid repositories.

## Solution

### Error Classification

The implementation adds a new error code `GitRepositoryTrust` to the `gitbutler-error::Code` enum:

```rust
pub enum Code {
    // ... existing codes
    GitRepositoryTrust,
}
```

This error code is mapped to the frontend-facing string `"errors.git.repository_trust"`.

### Repository Opening Logic

A new module `gitbutler-project/src/repository.rs` provides trust-aware repository opening:

```rust
pub enum RepositoryOpenResult {
    Success(gix::Repository),
    TrustError { error: gix::open::Error, path: PathBuf },
    OtherError(gix::open::Error),
}

// Opens with isolated security (default behavior)
pub fn open_repository_with_trust_check<P: AsRef<Path>>(path: P) -> RepositoryOpenResult

// Opens with full trust (bypasses security checks)
pub fn open_repository_with_full_trust<P: AsRef<Path>>(path: P) -> Result<gix::Repository>
```

### API Changes

New functions are available for adding projects with trust handling:

```rust
// Standard API (existing behavior)
pub fn add_project(path: P, name: Option<String>, email: Option<String>) -> Result<Project>

// New API for trust override
pub fn add_project_with_trust(path: P, name: Option<String>, email: Option<String>) -> Result<Project>
```

### Tauri Commands

New Tauri command is available for the frontend:

```rust
#[tauri::command(async)]
pub fn add_project_with_trust(
    app: State<'_, but_api::App>,
    path: &path::Path,
) -> Result<gitbutler_project::Project, Error>
```

## Usage Flow

### 1. Normal Repository Addition

```typescript
// Frontend pseudocode
try {
    const project = await invoke('add_project', { path: '/path/to/repo' });
    // Success - repository added normally
} catch (error) {
    if (error.code === 'errors.git.repository_trust') {
        // Handle trust error (see below)
    } else {
        // Handle other errors
    }
}
```

### 2. Trust Error Handling

When a trust error occurs:

```typescript
// Frontend pseudocode
try {
    const project = await invoke('add_project', { path: '/path/to/repo' });
} catch (error) {
    if (error.code === 'errors.git.repository_trust') {
        // Show user-friendly dialog
        const userApproves = await showTrustDialog({
            message: error.message,
            path: path,
            risks: [
                'Repository may be owned by a different user',
                'Repository may be on a network drive',
                'Repository may be in a shared folder'
            ]
        });
        
        if (userApproves) {
            // Retry with full trust
            const project = await invoke('add_project_with_trust', { path: '/path/to/repo' });
            // Success with trust override
        } else {
            // Show alternative solutions
            showAlternativeSolutions(path);
        }
    }
}
```

### 3. Alternative Solutions

If the user declines to open with full trust, provide guidance:

```typescript
function showAlternativeSolutions(repoPath: string) {
    const instructions = [
        `Configure Git's safe directory setting:`,
        `git config --global --add safe.directory '${repoPath}'`,
        ``,
        `Or change the repository ownership to match your user account.`
    ];
    showInstructionsDialog(instructions);
}
```

## Security Considerations

### Default Behavior

- **No change to existing security**: Default behavior remains unchanged
- **Isolated options**: Repositories are opened with `gix::open::Options::isolated()` by default
- **User consent required**: Full trust is only used when explicitly approved by the user

### Trust Override Behavior

- **Explicit user action**: Full trust requires a separate function call
- **Informed consent**: Users are informed about the security implications
- **Limited scope**: Trust override only applies to the specific repository being added

### User Education

The frontend should clearly communicate:

1. **What the error means**: Repository is in an untrusted location
2. **Why it happened**: Git's security mechanisms are protecting the user
3. **Risks of overriding**: Potential security implications of using full trust
4. **Alternative solutions**: How to configure Git to trust the repository

## Error Messages

### Backend Error

The backend provides a descriptive error with the `GitRepositoryTrust` code:

```
Repository opening failed due to Git security/trust settings. 
The repository may be in an untrusted location.
```

### Frontend Presentation

The frontend should expand this with user-friendly explanations:

```
Git Security Protection Active

This repository is located in a directory that Git considers potentially unsafe:
• The repository may be owned by a different user
• The repository may be on a network drive  
• The repository may be in a shared folder

GitButler can open this repository with full trust, but this bypasses 
Git's security protections.

[Open with Full Trust] [Cancel] [Show Instructions]
```

## Implementation Files

### Core Changes
- `crates/gitbutler-error/src/error.rs` - Added `GitRepositoryTrust` error code
- `crates/gitbutler-project/src/repository.rs` - New trust-aware repository opening
- `crates/gitbutler-project/src/controller.rs` - Updated to use trust checking
- `crates/gitbutler-project/src/lib.rs` - Added public API functions

### API Layer
- `crates/but-api/src/commands/projects.rs` - Added `add_project_with_trust` command
- `crates/gitbutler-tauri/src/projects.rs` - Added Tauri command wrapper
- `crates/gitbutler-tauri/src/main.rs` - Registered new Tauri command

## Testing

The implementation includes:

- Unit tests for trust error classification
- Integration tests for repository opening with different trust levels
- Example frontend implementation demonstrating the complete user flow

## Future Enhancements

### Potential Improvements

1. **Remember User Choice**: Store user's trust decisions for specific repositories
2. **Granular Trust Levels**: Support different trust levels beyond just "full"
3. **Repository Validation**: Additional validation of trusted repositories
4. **Admin Configuration**: Allow administrators to pre-configure trusted locations

### Configuration Options

Future versions could support:

```json
{
    "trustedDirectories": ["/safe/path/one", "/safe/path/two"],
    "autoTrustUserRepositories": true,
    "requireConfirmationForTrust": true
}
```

## Migration Guide

### For Existing Installations

No changes are required for existing installations. The new functionality:

- Maintains backward compatibility
- Only activates when trust errors occur
- Provides graceful fallback options

### For Frontend Developers

To support the new functionality:

1. **Add error handling** for `errors.git.repository_trust` error code
2. **Implement user dialog** for trust approval
3. **Add fallback to** `add_project_with_trust` command when approved
4. **Provide educational content** about Git security and alternatives

## Conclusion

This implementation provides a robust solution for Git repository trust issues while maintaining security and providing user education. The approach balances usability with safety, ensuring users can access their repositories while understanding the security implications of their choices.
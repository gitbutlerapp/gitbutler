# but-gitea

Minimal Gitea authentication support for GitButler.

This crate intentionally covers a narrow, reviewable slice of the eventual Gitea
integration work:

- validate a personal access token against a Gitea-compatible host
- fetch the authenticated user from `/api/v1/user`
- persist per-host account metadata in `but-forge-storage`
- persist the token itself in OS-backed secure storage via `but-secret`
- export the account and auth response types to TypeScript for the desktop UI

It does not currently implement repository discovery, remote detection, pull
request flows, or OAuth.

## Module layout

- `client.rs`: thin authenticated client for the small auth surface we need now
- `token.rs`: account identifiers plus secure token persistence helpers
- `lib.rs`: high-level account management API used by `but-api`

## Manual verification

The auth flow was designed to be easy to smoke-test against a real local Gitea
instance.

1. Start a local Gitea 1.25.x server.
2. Create a user and personal access token with `all` scope.
3. Call `store_account()` with the instance URL and PAT.
4. Confirm `list_known_gitea_accounts()` returns `username@host`.
5. Confirm `get_gitea_user()` reloads the stored account and fetches the live
   user profile.
6. Confirm `forget_gitea_access_token()` removes the account and token.

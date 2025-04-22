export const KNOWN_ERRORS: Record<string, string> = {
	'errors.commit.signing_failed': `
			Commit signing failed and has now been disabled. You can configure commit signing in the project settings.

			Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/verifying-commits) on setting up commit signing and verification.
		`
};

import { Code } from '$lib/backend/ipc';

export const KNOWN_ERRORS: Record<string, string> = {
	[Code.CommitSigningFailed]: `
Commit signing failed and has now been disabled. You can configure commit signing in the project settings.

Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/signing-commits) on setting up commit signing and verification.
		`,
	[Code.SecretKeychainNotFound]: `
Please install a keychain service to store and retrieve secrets with.

This can be done using \`sudo apt install gnome-keyring\` for instance.
	`
};

export enum Code {
	Unknown = 'errors.unknown',
	Validation = 'errors.validation',
	ProjectsGitAuth = 'errors.projects.git.auth',
	DefaultTargetNotFound = 'errors.projects.default_target.not_found',
	CommitSigningFailed = 'errors.commit.signing_failed',
	ProjectMissing = 'errors.projects.missing',
	SecretKeychainNotFound = 'errors.secret.keychain_notfound',
	MissingLoginKeychain = 'errors.secret.missing_login_keychain',
	GitHubTokenExpired = 'errors.github.expired_token'
}

export const KNOWN_ERRORS: Record<string, string> = {
	[Code.CommitSigningFailed]: `
Commit signing failed and has now been disabled. You can configure commit signing in the project settings.

Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/signing-commits) on setting up commit signing and verification.
		`,
	[Code.SecretKeychainNotFound]: `
Please install a keychain service to store and retrieve secrets with.

This can be done using \`sudo apt install gnome-keyring\` for instance.
	`,
	[Code.MissingLoginKeychain]: `
Missing default keychain.

With \`seahorse\` or equivalent, create a \`Login\` password store, right click it and choose \`Set Default\`.
	`,
	[Code.GitHubTokenExpired]: `
Your GitHub token appears expired, please check your settings!
	`
};

export enum Code {
	Unknown = 'errors.unknown',
	Validation = 'errors.validation',
	RepoOwnership = 'errors.repo_ownership',
	ProjectsGitAuth = 'errors.projects.git.auth',
	DefaultTargetNotFound = 'errors.projects.default_target.not_found',
	CommitSigningFailed = 'errors.commit.signing_failed',
	ProjectMissing = 'errors.projects.missing',
	SecretKeychainNotFound = 'errors.secret.keychain_notfound',
	MissingLoginKeychain = 'errors.secret.missing_login_keychain',
	GitHubTokenExpired = 'errors.github.expired_token',
	GitHubStackedPrFork = 'errors.github.stacked_pr_fork'
}

export const KNOWN_ERRORS: Record<string, string> = {
	[Code.CommitSigningFailed]: `
Commit signing failed and has now been disabled. You can configure commit signing in the project settings.

Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/signing-commits) on setting up commit signing and verification.
		`,
	[Code.RepoOwnership]: `
The repository ownership couldn't be determined. Consider allowing it using:

    git config --global --add safe.directory copy/of/path/shown/below
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
	`,
	[Code.GitHubStackedPrFork]: `
Stacked pull requests across forks are not supported by GitHub.

The base branch you specified doesn't exist in your fork. When creating a stacked PR, the base branch must exist in the same repository.
	`
};

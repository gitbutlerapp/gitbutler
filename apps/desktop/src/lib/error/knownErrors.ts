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
	ProjectDatabaseIncompatible = 'errors.projectdb.migration',
	DefaultTerminalNotFound = 'errors.terminal.not_found',
	EditModeSafeCheckoutFailed = 'errors.edit_mode.safe_checkout_failed'
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
Your GitHub token appears expired. Please log out and back in to refresh it. (Settings -> Integrations -> Forget) 
	`,
	[Code.ProjectDatabaseIncompatible]: `
The database was changed by a more recent version of GitButler - cannot safely open it anymore.
	`,
	[Code.DefaultTerminalNotFound]: `
Your default terminal was not found. Please select your preferred terminal in Settings > General.
The database was changed by a more recent version of GitButler - cannot safely open it anymore. 
	`,
	[Code.EditModeSafeCheckoutFailed]: `
GitButler couldn't safely return to the workspace because doing so would overwrite local changes.

Please manually stash or save your changes and try again.
  `
};

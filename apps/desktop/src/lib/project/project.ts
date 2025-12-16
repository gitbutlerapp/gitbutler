import { goto } from '$app/navigation';
import { showToast } from '$lib/notifications/toasts';
import { projectPath } from '$lib/routes/routes.svelte';
import { TestId } from '@gitbutler/ui';
import type { GitHubAccountIdentifier } from '$lib/forge/github/githubUserService.svelte';
import type { ForgeName } from '$lib/forge/interface/forge';

export type KeyType = 'gitCredentialsHelper' | 'local' | 'systemExecutable';
export type LocalKey = {
	local: { private_key_path: string };
};

export type AuthKey = Exclude<KeyType, 'local'> | LocalKey;

export type ForgeUserIdentifier = {
	provider: 'github';
	details: GitHubAccountIdentifier;
};

export type Project = {
	id: string;
	title: string;
	description?: string;
	path: string;
	git_dir?: string;
	api?: CloudProject & {
		sync: boolean;
		sync_code: boolean | undefined;
		reviews: boolean | undefined;
	};
	preferred_key: AuthKey;
	ok_with_force_push: boolean;
	force_push_protection: boolean;
	omit_certificate_check: boolean | undefined;
	use_diff_context: boolean | undefined;
	// Produced just for the frontend to determine if the project is open in any window.
	is_open: boolean;
	forge_override: ForgeName | undefined;
	preferred_forge_user: ForgeUserIdentifier | null;
	// Gerrit mode enabled for this project, derived from git configuration
	gerrit_mode: boolean;
	/**
	 * The path to the forge review template, if set in git configuration.
	 */
	forge_review_template_path: string | null;
};

export function vscodePath(path: string) {
	return path.includes('\\') ? '/' + path.replace('\\', '/') : path;
}

export function gitAuthType(preferredKey?: AuthKey): string {
	if (typeof preferredKey === 'object' && preferredKey !== null && 'local' in preferredKey) {
		return 'local';
	}
	return preferredKey as KeyType;
}

export type CloudProject = {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	code_git_url: string;
	created_at: string;
	updated_at: string;
};

export type AddProjectOutcome =
	| {
			type: 'added';
			subject: Project;
	  }
	| {
			type: 'alreadyExists';
			subject: Project;
	  }
	| {
			type: 'pathNotFound';
	  }
	| {
			type: 'notADirectory';
	  }
	| {
			type: 'bareRepository';
	  }
	| {
			type: 'nonMainWorktree';
	  }
	| {
			type: 'noWorkdir';
	  }
	| {
			type: 'noDotGitDirectory';
	  }
	| {
			type: 'notAGitRepository';
			/**
			 * The error message received
			 */
			subject: string;
	  };

/**
 * Correctly handle the outcome of an addProject operation by passing the project to the callback or
 * showing toasts as necessary.
 */
export function handleAddProjectOutcome(
	outcome: AddProjectOutcome,
	onAdded?: (project: Project) => void
): true {
	switch (outcome.type) {
		case 'added':
			onAdded?.(outcome.subject);
			return true;
		case 'alreadyExists':
			showToast({
				testId: TestId.AddProjectAlreadyExistsModal,
				style: 'warning',
				title: `Project '${outcome.subject.title}' already exists`,
				message: `The project at "${outcome.subject.path}" is already added`,
				extraAction: {
					label: 'Open project',
					testId: TestId.AddProjectAlreadyExistsModalOpenProjectButton,
					onClick: (dismiss) => {
						goto(projectPath(outcome.subject.id));
						dismiss();
					}
				}
			});
			return true;
		case 'pathNotFound':
			showToast({
				style: 'warning',
				title: 'Path not found',
				message: 'The specified path does not exist on the filesystem.'
			});
			return true;
		case 'notADirectory':
			showToast({
				style: 'warning',
				title: 'Not a directory',
				message: 'The specified path is not a directory.'
			});
			return true;
		case 'bareRepository':
			showToast({
				testId: TestId.AddProjectBareRepoModal,
				style: 'danger',
				title: 'Bare repository',
				message: 'The specified path appears to be a bare Git repository and cannot be added.'
			});
			return true;
		case 'nonMainWorktree':
			showToast({
				style: 'warning',
				title: 'Non-main worktree',
				message: 'The specified path is not the main worktree of the repository.'
			});
			return true;
		case 'noWorkdir':
			showToast({
				style: 'warning',
				title: 'No workdir',
				message: 'The specified repository does not have a working directory.'
			});
			return true;
		case 'noDotGitDirectory':
			showToast({
				testId: TestId.AddProjectNoDotGitDirectoryModal,
				style: 'warning',
				title: 'No .git directory',
				message: 'The specified path does not contain a .git directory.'
			});
			return true;
		case 'notAGitRepository':
			showToast({
				testId: TestId.AddProjectNotAGitRepoModal,
				style: 'warning',
				title: 'Not a Git repository',
				message: `Unable to add project: ${outcome.subject}`
			});
			return true;
	}
}

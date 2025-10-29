import {
	GITHUB_USER_SERVICE,
	type GitHubAccountIdentifier
} from '$lib/forge/github/githubUserService.svelte';
import { PROJECTS_SERVICE } from '$lib/project/projectsService';
import { inject } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

type GitHubPreferences = {
	preferredGitHubAccount: Reactive<GitHubAccountIdentifier | undefined>;
	githubAccounts: Reactive<GitHubAccountIdentifier[]>;
};

/**
 * Return the preferred GitHub username for the given project ID, based on the known
 * GitHub usernames in the application settings and the project's preferred forge user.
 */
export function usePreferredGitHubUsername(projectId: Reactive<string>): GitHubPreferences {
	const githubUserService = inject(GITHUB_USER_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const githubUsernamesResponse = githubUserService.accounts();
	const githubAccounts = $derived(githubUsernamesResponse?.response ?? []);

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		if (githubAccounts.length === 0) {
			return undefined;
		}
		return (
			githubAccounts.find((account) => account.info.username === project?.preferred_forge_user) ??
			githubAccounts.at(0)
		);
	});

	return {
		preferredGitHubAccount: reactive(() => preferredUser),
		githubAccounts: reactive(() => githubAccounts)
	};
}

type GitHubAccess = {
	accessToken: Reactive<string | undefined>;
	isLoading: Reactive<boolean>;
};

/**
 * Return the GitHub access token for the given project ID, based on the preferred GitHub username.
 */
export function useGitHubAccessToken(projectId: Reactive<string>): GitHubAccess {
	const githubUserService = inject(GITHUB_USER_SERVICE);
	const { preferredGitHubAccount } = usePreferredGitHubUsername(projectId);
	const ghUserResponse = $derived.by(() => {
		if (preferredGitHubAccount.current === undefined) return undefined;
		return githubUserService.authenticatedUser(preferredGitHubAccount.current);
	});
	const aceessToken = $derived(ghUserResponse?.response?.accessToken);
	return {
		accessToken: reactive(() => aceessToken),
		isLoading: reactive(() => ghUserResponse?.result.isLoading ?? false)
	};
}

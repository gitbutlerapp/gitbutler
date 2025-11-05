import {
	GITHUB_USER_SERVICE,
	isSameGitHubAccountIdentifier,
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

		if (
			project === undefined ||
			project.preferred_forge_user === null ||
			project.preferred_forge_user.provider !== 'github'
		) {
			return githubAccounts.at(0);
		}

		const preferredForgeUser = project.preferred_forge_user.details;

		return (
			githubAccounts.find((account) =>
				isSameGitHubAccountIdentifier(account, preferredForgeUser)
			) ?? githubAccounts.at(0)
		);
	});

	return {
		preferredGitHubAccount: reactive(() => preferredUser),
		githubAccounts: reactive(() => githubAccounts)
	};
}

type GitHubAccess = {
	host: Reactive<string | undefined>;
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
	const host = $derived.by(() => {
		if (preferredGitHubAccount.current?.type === 'enterprise') {
			return preferredGitHubAccount.current.info.host;
		}
		return undefined;
	});
	return {
		host: reactive(() => host),
		accessToken: reactive(() => aceessToken),
		isLoading: reactive(() => ghUserResponse?.result.isLoading ?? false)
	};
}

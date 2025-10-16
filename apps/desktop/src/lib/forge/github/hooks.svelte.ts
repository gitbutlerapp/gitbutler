import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
import { PROJECTS_SERVICE } from '$lib/project/projectsService';
import { inject } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

type GitHubPreferences = {
	preferredGitHubUsername: Reactive<string | undefined>;
	githubUsernames: Reactive<string[]>;
};

/**
 * Return the preferred GitHub username for the given project ID, based on the known
 * GitHub usernames in the application settings and the project's preferred forge user.
 */
export function usePreferredGitHubUsername(projectId: Reactive<string>): GitHubPreferences {
	let githubUsernames = $state<string[]>([]);
	const appSettings = inject(SETTINGS_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const knownGitHubUsernames = $derived(appSettings.knownGitHubUsernames);
	knownGitHubUsernames.subscribe((value) => {
		githubUsernames = value;
	});

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		if (githubUsernames.length === 0) {
			return undefined;
		}
		return (
			githubUsernames.find((u) => u === project?.preferred_forge_user) ?? githubUsernames.at(0)
		);
	});

	return {
		preferredGitHubUsername: reactive(() => preferredUser),
		githubUsernames: reactive(() => githubUsernames)
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
	const { preferredGitHubUsername } = usePreferredGitHubUsername(projectId);
	const ghUserResponse = $derived.by(() => {
		if (preferredGitHubUsername.current === undefined) return undefined;
		return githubUserService.authenticatedUser(preferredGitHubUsername.current);
	});
	const aceessToken = $derived(ghUserResponse?.response?.accessToken);
	return {
		accessToken: reactive(() => aceessToken),
		isLoading: reactive(() => ghUserResponse?.result.isLoading ?? false)
	};
}

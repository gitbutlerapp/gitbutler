import {
	GITLAB_USER_SERVICE,
	isSameGitLabAccountIdentifier,
} from "$lib/forge/gitlab/gitlabUserService.svelte";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { ButGitLabToken } from "@gitbutler/core/api";
import type { Reactive } from "@gitbutler/shared/storeUtils";

type GitLabPreferences = {
	preferredGitLabAccount: Reactive<ButGitLabToken.GitlabAccountIdentifier | undefined>;
	gitlabAccounts: Reactive<ButGitLabToken.GitlabAccountIdentifier[]>;
};

/**
 * Return the preferred GitLab username for the given project ID, based on the known
 * GitLab usernames in the application settings and the project's preferred forge user.
 */
export function usePreferredGitLabUsername(projectId: Reactive<string>): GitLabPreferences {
	const gitlabUserService = inject(GITLAB_USER_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const gitlabUsernamesResponse = gitlabUserService.accounts();
	const gitlabAccounts = $derived(gitlabUsernamesResponse?.response ?? []);

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		if (gitlabAccounts.length === 0) {
			return undefined;
		}

		if (
			project === undefined ||
			project.preferred_forge_user === null ||
			project.preferred_forge_user.provider !== "gitlab"
		) {
			return gitlabAccounts.at(0);
		}

		const preferredForgeUser = project.preferred_forge_user.details;

		return (
			gitlabAccounts.find((account) =>
				isSameGitLabAccountIdentifier(account, preferredForgeUser),
			) ?? gitlabAccounts.at(0)
		);
	});

	return {
		preferredGitLabAccount: reactive(() => preferredUser),
		gitlabAccounts: reactive(() => gitlabAccounts),
	};
}

type GitLabAccess = {
	host: Reactive<string | undefined>;
	accessToken: Reactive<string | undefined>;
	isLoading: Reactive<boolean>;
	error: Reactive<{ code: string; message: string } | undefined>;
	isError: Reactive<boolean>;
};

/**
 * Return the GitLab access token for the given project ID, based on the preferred GitLab username.
 */
export function useGitLabAccessToken(projectId: Reactive<string>): GitLabAccess {
	const gitlabUserService = inject(GITLAB_USER_SERVICE);
	const { preferredGitLabAccount } = usePreferredGitLabUsername(projectId);
	const glUserResponse = $derived.by(() => {
		if (preferredGitLabAccount.current === undefined) return undefined;
		return gitlabUserService.authenticatedUser(preferredGitLabAccount.current);
	});
	const aceessToken = $derived(glUserResponse?.response?.accessToken);
	const host = $derived.by(() => {
		if (preferredGitLabAccount.current?.type === "selfHosted") {
			return preferredGitLabAccount.current.info.host;
		}
		return undefined;
	});
	return {
		host: reactive(() => host),
		accessToken: reactive(() => aceessToken),
		isLoading: reactive(() => glUserResponse?.result.isLoading ?? false),
		error: reactive(
			() => glUserResponse?.result.error as { code: string; message: string } | undefined,
		),
		isError: reactive(() => glUserResponse?.result.isError ?? false),
	};
}

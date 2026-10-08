import {
	GITEE_USER_SERVICE,
	isSameGiteeAccountIdentifier,
} from "$lib/forge/gitee/giteeUserService.svelte";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { GiteeAccountIdentifier } from "@gitbutler/but-sdk";
import type { Reactive } from "@gitbutler/shared/storeUtils";

type GiteePreferences = {
	preferredGiteeAccount: Reactive<GiteeAccountIdentifier | undefined>;
	giteeAccounts: Reactive<GiteeAccountIdentifier[]>;
};

/**
 * Return the preferred Gitee account for the given project ID, based on the known
 * Gitee accounts in the application settings and the project's preferred forge user.
 */
export function usePreferredGiteeUsername(projectId: Reactive<string>): GiteePreferences {
	const giteeUserService = inject(GITEE_USER_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const giteeAccountsResponse = giteeUserService.accounts();
	const giteeAccounts = $derived(giteeAccountsResponse?.response ?? []);

	const projectQuery = $derived(projectsService.getProject(projectId.current));
	const project = $derived(projectQuery.response);
	const preferredUser = $derived.by(() => {
		if (giteeAccounts.length === 0) {
			return undefined;
		}

		if (
			project === undefined ||
			project.preferred_forge_user === null ||
			project.preferred_forge_user.provider !== "gitee"
		) {
			return giteeAccounts.at(0);
		}

		const preferredForgeUser = project.preferred_forge_user.details;

		return (
			giteeAccounts.find((account) => isSameGiteeAccountIdentifier(account, preferredForgeUser)) ??
			giteeAccounts.at(0)
		);
	});

	return {
		preferredGiteeAccount: reactive(() => preferredUser),
		giteeAccounts: reactive(() => giteeAccounts),
	};
}

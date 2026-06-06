import {
	GITHUB_USER_SERVICE,
	isSameGitHubAccountIdentifier,
} from "$lib/forge/github/githubUserService.svelte";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { inject } from "@gitbutler/core/context";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import type { ForgeUserQuery } from "$lib/forge/interface/types";
import type { Code, GithubAccountIdentifier } from "@gitbutler/but-sdk";
import type { Reactive } from "@gitbutler/shared/storeUtils";

type GitHubPreferences = {
	preferredGitHubAccount: Reactive<GithubAccountIdentifier | undefined>;
	githubAccounts: Reactive<GithubAccountIdentifier[]>;
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
			project.preferred_forge_user.provider !== "github"
		) {
			return githubAccounts.at(0);
		}

		const preferredForgeUser = project.preferred_forge_user.details;

		return (
			githubAccounts.find((account) =>
				isSameGitHubAccountIdentifier(account, preferredForgeUser),
			) ?? githubAccounts.at(0)
		);
	});

	return {
		preferredGitHubAccount: reactive(() => preferredUser),
		githubAccounts: reactive(() => githubAccounts),
	};
}

type GitHubAccess = {
	host: Reactive<string | undefined>;
	accessToken: Reactive<string | undefined>;
	isLoading: Reactive<boolean>;
	error: Reactive<{ code?: Code; message: string } | undefined>;
	isError: Reactive<boolean>;
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
		if (preferredGitHubAccount.current?.type === "enterprise") {
			return preferredGitHubAccount.current.info.host;
		}
		return undefined;
	});
	return {
		host: reactive(() => host),
		accessToken: reactive(() => aceessToken),
		isLoading: reactive(() => ghUserResponse?.result.isLoading ?? false),
		error: reactive(
			() => ghUserResponse?.result.error as { code?: Code; message: string } | undefined,
		),
		isError: reactive(() => ghUserResponse?.result.isError ?? false),
	};
}

/**
 * Resolve the project's preferred GitHub account and fetch it as a
 * display-ready `ForgeUser`. `user` is `undefined` when no GitHub
 * account is configured.
 *
 * `inject()` runs once at call time, so this must be invoked during
 * component init — not inside a `$derived`. The account lookup and the
 * user query are built reactively inside, so the result still updates
 * when the preferred account resolves.
 */
export function useGitHubForgeUser(projectId: Reactive<string>): ForgeUserQuery {
	const githubUserService = inject(GITHUB_USER_SERVICE);
	const { preferredGitHubAccount } = usePreferredGitHubUsername(projectId);
	const userQuery = $derived.by(() => {
		const account = preferredGitHubAccount.current;
		if (account === undefined) return undefined;
		return githubUserService.authenticatedUser(account, {
			transform: (result) =>
				result
					? {
							login: result.login,
							name: result.name ?? result.login,
							srcUrl: result.avatarUrl ?? "",
						}
					: undefined,
		});
	});
	return {
		user: reactive(() => userQuery?.response),
		isLoading: reactive(() => userQuery?.result.isLoading ?? false),
	};
}

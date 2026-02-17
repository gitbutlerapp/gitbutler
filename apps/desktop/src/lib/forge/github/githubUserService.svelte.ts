import { providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { ButGitHub, ButGitHubToken } from "@gitbutler/core/api";

export const GITHUB_USER_SERVICE = new InjectionToken<GitHubUserService>("GitHubUserService");

type Verification = {
	user_code: string;
	device_code: string;
};

export function isSameGitHubAccountIdentifier(
	a: ButGitHubToken.GithubAccountIdentifier,
	b: ButGitHubToken.GithubAccountIdentifier,
): boolean {
	if (a.type !== b.type) {
		return false;
	}
	switch (a.type) {
		case "oAuthUsername":
		case "patUsername":
			return a.info.username === (b as typeof a).info.username;
		case "enterprise":
			return (
				a.info.host === (b as typeof a).info.host &&
				a.info.username === (b as typeof a).info.username
			);
	}
}

export type GitHubAccountIdentifierType = ButGitHubToken.GithubAccountIdentifier["type"];

function isGitHubAccountIdentifierType(text: unknown): text is GitHubAccountIdentifierType {
	if (typeof text !== "string") {
		return false;
	}
	return text === "oAuthUsername" || text === "patUsername" || text === "enterprise";
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function githubAccountIdentifierToString(
	account: ButGitHubToken.GithubAccountIdentifier,
): string {
	switch (account.type) {
		case "oAuthUsername":
			return `${account.type}${UNIT_SEP}${account.info.username}`;
		case "patUsername":
			return `${account.type}${UNIT_SEP}${account.info.username}`;
		case "enterprise":
			return `${account.type}${UNIT_SEP}${account.info.host}${UNIT_SEP}${account.info.username}`;
	}
}

export function stringToGitHubAccountIdentifier(
	str: string,
): ButGitHubToken.GithubAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length < 2) {
		return null;
	}
	const [type, ...infoParts] = parts;

	if (!isGitHubAccountIdentifierType(type)) {
		return null;
	}

	switch (type) {
		case "oAuthUsername":
			if (infoParts.length < 1) return null;
			return {
				type: "oAuthUsername",
				info: {
					username: infoParts[0]!,
				},
			};
		case "patUsername":
			if (infoParts.length < 1) return null;

			return {
				type: "patUsername",
				info: {
					username: infoParts[0]!,
				},
			};
		case "enterprise":
			if (infoParts.length < 2) return null;

			return {
				type: "enterprise",
				info: {
					host: infoParts[0]!,
					username: infoParts[1]!,
				},
			};
	}
}

export class GitHubUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(backendApi: BackendApi) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	async initDeviceOauth() {
		return await this.backendApi.endpoints.initDeviceOauth.mutate();
	}

	async checkAuthStatus(params: { deviceCode: string }) {
		return await this.backendApi.endpoints.checkAuthStatus.mutate(params);
	}

	get storeGitHubPat() {
		return this.backendApi.endpoints.storeGitHubPat.useMutation();
	}

	get storeGithuibEnterprisePat() {
		return this.backendApi.endpoints.storeGithuibEnterprisePat.useMutation();
	}

	get forgetGitHubUsername() {
		return this.backendApi.endpoints.forgetGitHubAccount.useMutation();
	}

	authenticatedUser(account: ButGitHubToken.GithubAccountIdentifier) {
		return this.backendApi.endpoints.getGitHubUser.useQuery({ account });
	}

	accounts() {
		return this.backendApi.endpoints.listKnownGitHubAccounts.useQuery();
	}
	deleteAllGitHubAccounts() {
		return this.backendApi.endpoints.clearAllGitHubAccounts.useMutation();
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetGitHubAccount: build.mutation<void, ButGitHubToken.GithubAccountIdentifier>({
				extraOptions: {
					command: "forget_github_account",
					actionName: "Forget GitHub Username",
				},
				query: (account) => ({
					account,
				}),
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
			initDeviceOauth: build.mutation<Verification, void>({
				extraOptions: {
					command: "init_github_device_oauth",
					actionName: "Init GitHub Device OAuth",
				},
				query: () => ({}),
			}),
			checkAuthStatus: build.mutation<
				ButGitHub.AuthStatusResponseSensitive,
				{ deviceCode: string }
			>({
				extraOptions: {
					command: "check_github_auth_status",
					actionName: "Check GitHub Auth Status",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
			getGitHubUser: build.query<
				ButGitHub.AuthenticatedUserSensitive | null,
				{ account: ButGitHubToken.GithubAccountIdentifier }
			>({
				extraOptions: {
					command: "get_gh_user",
				},
				query: (args) => args,
				providesTags: (_result, _error, username) => [
					...providesItem(ReduxTag.ForgeUser, `github:${username}`),
				],
			}),
			listKnownGitHubAccounts: build.query<ButGitHubToken.GithubAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_github_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
			clearAllGitHubAccounts: build.mutation<void, void>({
				extraOptions: {
					command: "clear_all_github_tokens",
					actionName: "Clear All GitHub Accounts",
				},
				query: () => ({}),
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
			storeGitHubPat: build.mutation<
				ButGitHub.AuthStatusResponseSensitive,
				{ accessToken: string }
			>({
				extraOptions: {
					command: "store_github_pat",
					actionName: "Store GitHub PAT",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
			storeGithuibEnterprisePat: build.mutation<
				ButGitHub.AuthStatusResponseSensitive,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_github_enterprise_pat",
					actionName: "Store GitHub Enterprise PAT",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)],
			}),
		}),
	});
}

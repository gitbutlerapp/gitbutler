import { providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { SecretsService } from "$lib/secrets/secretsService";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { ButGitLab, ButGitLabToken } from "@gitbutler/core/api";

export const GITLAB_USER_SERVICE = new InjectionToken<GitLabUserService>("GitLabUserService");

export function isSameGitLabAccountIdentifier(
	a: ButGitLabToken.GitlabAccountIdentifier,
	b: ButGitLabToken.GitlabAccountIdentifier,
): boolean {
	if (a.type !== b.type) {
		return false;
	}
	switch (a.type) {
		case "patUsername":
			return a.info.username === (b as typeof a).info.username;
		case "selfHosted":
			return (
				a.info.host === (b as typeof a).info.host &&
				a.info.username === (b as typeof a).info.username
			);
	}
}

export type GitLabAccountIdentifierType = ButGitLabToken.GitlabAccountIdentifier["type"];

function isGitLabAccountIdentifierType(text: unknown): text is GitLabAccountIdentifierType {
	if (typeof text !== "string") {
		return false;
	}
	return text === "patUsername" || text === "enterprise";
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function gitlabAccountIdentifierToString(
	account: ButGitLabToken.GitlabAccountIdentifier,
): string {
	switch (account.type) {
		case "patUsername":
			return `${account.type}${UNIT_SEP}${account.info.username}`;
		case "selfHosted":
			return `${account.type}${UNIT_SEP}${account.info.host}${UNIT_SEP}${account.info.username}`;
	}
}

export function stringToGitLabAccountIdentifier(
	str: string,
): ButGitLabToken.GitlabAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length < 2) {
		return null;
	}
	const [type, ...infoParts] = parts;

	if (!isGitLabAccountIdentifierType(type)) {
		return null;
	}

	switch (type) {
		case "patUsername":
			if (infoParts.length < 1) return null;

			return {
				type: "patUsername",
				info: {
					username: infoParts[0]!,
				},
			};
		case "selfHosted":
			if (infoParts.length < 2) return null;

			return {
				type: "selfHosted",
				info: {
					host: infoParts[0]!,
					username: infoParts[1]!,
				},
			};
	}
}

export class GitLabUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		backendApi: BackendApi,
		private secretsService: SecretsService,
	) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	/**
	 * Migrate the access token for the given project from the old storage location (if it exists) to the new one.
	 */
	async migrate(projectId: string): Promise<void> {
		try {
			const gitlabToken = await this.secretsService.get(`git-lab-token:${projectId}`);
			if (!gitlabToken) return;
			await this.backendApi.endpoints.storeGitLabPat.initiate({ accessToken: gitlabToken });
			await this.secretsService.delete(`git-lab-token:${projectId}`);
		} catch (error) {
			// Fail should not explote. Log instead.
			console.warn(`Failed to migrate GitLab token for project ${projectId}:`, error);
		}
	}

	get storeGitLabPat() {
		return this.backendApi.endpoints.storeGitLabPat.useMutation();
	}

	get storeGitLabEnterprisePat() {
		return this.backendApi.endpoints.storeGitLabEnterprisePat.useMutation();
	}

	get forgetGitLabAccount() {
		return this.backendApi.endpoints.forgetGitLabAccount.useMutation();
	}

	authenticatedUser(account: ButGitLabToken.GitlabAccountIdentifier) {
		return this.backendApi.endpoints.getGitLabUser.useQuery({ account });
	}

	accounts() {
		return this.backendApi.endpoints.listKnownGitLabAccounts.useQuery();
	}

	deleteAllGitLabAccounts() {
		return this.backendApi.endpoints.clearAllGitLabAccounts.useMutation();
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetGitLabAccount: build.mutation<void, ButGitLabToken.GitlabAccountIdentifier>({
				extraOptions: {
					command: "forget_gitlab_account",
					actionName: "Forget GitLab Account",
				},
				query: (account) => ({
					account,
				}),
				invalidatesTags: [providesList(ReduxTag.GitLabUserList)],
			}),
			getGitLabUser: build.query<
				ButGitLab.AuthenticatedUserSensitive | null,
				{ account: ButGitLabToken.GitlabAccountIdentifier }
			>({
				extraOptions: {
					command: "get_gl_user",
				},
				query: (args) => args,
				providesTags: (_result, _error, username) => [
					...providesItem(ReduxTag.ForgeUser, `gitlab:${username}`),
				],
			}),
			listKnownGitLabAccounts: build.query<ButGitLabToken.GitlabAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_gitlab_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GitLabUserList)],
			}),
			clearAllGitLabAccounts: build.mutation<void, void>({
				extraOptions: {
					command: "clear_all_gitlab_tokens",
					actionName: "Clear All GitLab Accounts",
				},
				query: () => ({}),
				invalidatesTags: [providesList(ReduxTag.GitLabUserList)],
			}),
			storeGitLabPat: build.mutation<
				ButGitLab.AuthStatusResponseSensitive,
				{ accessToken: string }
			>({
				extraOptions: {
					command: "store_gitlab_pat",
					actionName: "Store GitLab PAT",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitLabUserList)],
			}),
			storeGitLabEnterprisePat: build.mutation<
				ButGitLab.AuthStatusResponseSensitive,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_gitlab_selfhosted_pat",
					actionName: "Store GitLab Enterprise PAT",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitLabUserList)],
			}),
		}),
	});
}

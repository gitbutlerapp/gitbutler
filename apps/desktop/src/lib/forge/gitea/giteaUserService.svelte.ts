import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesList,
	ReduxTag,
} from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { ButGitea, ButGiteaToken } from "@gitbutler/core/api";

export const GITEA_USER_SERVICE = new InjectionToken<GiteaUserService>("GiteaUserService");

export function isSameGiteaAccountIdentifier(
	a: ButGiteaToken.GiteaAccountIdentifier,
	b: ButGiteaToken.GiteaAccountIdentifier,
): boolean {
	return a.host === b.host && a.username === b.username;
}

const UNIT_SEP = "\u001F";

export function giteaAccountIdentifierToString(
	account: ButGiteaToken.GiteaAccountIdentifier,
): string {
	return `${account.host}${UNIT_SEP}${account.username}`;
}

export function stringToGiteaAccountIdentifier(
	str: string,
): ButGiteaToken.GiteaAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length !== 2) {
		return null;
	}

	return {
		host: parts[0]!,
		username: parts[1]!,
	};
}

export class GiteaUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(backendApi: BackendApi) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	get storeGiteaAccount() {
		return this.backendApi.endpoints.storeGiteaAccount.useMutation();
	}

	get forgetGiteaAccount() {
		return this.backendApi.endpoints.forgetGiteaAccount.useMutation();
	}

	authenticatedUser(account: ButGiteaToken.GiteaAccountIdentifier) {
		return this.backendApi.endpoints.getGiteaUser.useQuery({ account });
	}

	accounts() {
		return this.backendApi.endpoints.listKnownGiteaAccounts.useQuery();
	}

	deleteAllGiteaAccounts() {
		return this.backendApi.endpoints.clearAllGiteaAccounts.useMutation();
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetGiteaAccount: build.mutation<void, ButGiteaToken.GiteaAccountIdentifier>({
				extraOptions: {
					command: "forget_gitea_account",
					actionName: "Forget Gitea Account",
				},
				query: (account) => ({
					account,
				}),
				invalidatesTags: (_result, _error, account) => [
					invalidatesList(ReduxTag.GiteaUserList),
					invalidatesItem(ReduxTag.ForgeUser, `gitea:${account.host}:${account.username}`),
				],
			}),
			getGiteaUser: build.query<
				ButGitea.AuthenticatedUserSensitive | null,
				{ account: ButGiteaToken.GiteaAccountIdentifier }
			>({
				extraOptions: {
					command: "get_gitea_user",
				},
				query: (args) => args,
				providesTags: (_result, _error, { account }) => [
					...providesItem(ReduxTag.ForgeUser, `gitea:${account.host}:${account.username}`),
				],
			}),
			listKnownGiteaAccounts: build.query<ButGiteaToken.GiteaAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_gitea_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GiteaUserList)],
			}),
			clearAllGiteaAccounts: build.mutation<void, void>({
				extraOptions: {
					command: "clear_all_gitea_tokens",
					actionName: "Clear All Gitea Accounts",
				},
				query: () => ({}),
				invalidatesTags: [providesList(ReduxTag.GiteaUserList)],
			}),
			storeGiteaAccount: build.mutation<
				ButGitea.AuthStatusResponseSensitive,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_gitea_account",
					actionName: "Store Gitea Account",
				},
				query: (args) => args,
				invalidatesTags: (result) => [
					invalidatesList(ReduxTag.GiteaUserList),
					...(result
						? [invalidatesItem(ReduxTag.ForgeUser, `gitea:${result.host}:${result.username}`)]
						: []),
				],
			}),
		}),
	});
}

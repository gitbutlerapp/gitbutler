import { providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";
import type { ButGitea, ButGiteaToken } from "@gitbutler/core/api";

export const GITEA_USER_SERVICE = new InjectionToken<GiteaUserService>("GiteaUserService");

export function isSameGiteaAccountIdentifier(
	a: ButGiteaToken.GiteaAccountIdentifier,
	b: ButGiteaToken.GiteaAccountIdentifier,
): boolean {
	if (a.type !== b.type) {
		return false;
	}
	switch (a.type) {
		case "selfHosted":
			return (
				a.info.host === (b as typeof a).info.host &&
				a.info.username === (b as typeof a).info.username
			);
	}
}

export type GiteaAccountIdentifierType = ButGiteaToken.GiteaAccountIdentifier["type"];

function isGiteaAccountIdentifierType(text: unknown): text is GiteaAccountIdentifierType {
	if (typeof text !== "string") {
		return false;
	}
	return text === "selfHosted";
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function giteaAccountIdentifierToString(
	account: ButGiteaToken.GiteaAccountIdentifier,
): string {
	switch (account.type) {
		case "selfHosted":
			return `${account.type}${UNIT_SEP}${account.info.host}${UNIT_SEP}${account.info.username}`;
	}
}

export function stringToGiteaAccountIdentifier(
	str: string,
): ButGiteaToken.GiteaAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length < 2) {
		return null;
	}
	const [type, ...infoParts] = parts;

	if (!isGiteaAccountIdentifierType(type)) {
		return null;
	}

	switch (type) {
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

export class GiteaUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(backendApi: BackendApi) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	get storeGiteaPat() {
		return this.backendApi.endpoints.storeGiteaPat.useMutation();
	}

	get forgetGiteaUsername() {
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

	checkGiteaCredentials(account: ButGiteaToken.GiteaAccountIdentifier) {
		return this.backendApi.endpoints.checkGiteaCredentials.useMutation();
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
				invalidatesTags: [providesList(ReduxTag.GiteaUserList)],
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
					...providesItem(ReduxTag.ForgeUser, `gitea:${account.info.host}:${account.info.username}`),
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
			storeGiteaPat: build.mutation<
				ButGitea.AuthStatusResponseSensitive,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_gitea_pat",
					actionName: "Store Gitea PAT",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GiteaUserList)],
			}),
			checkGiteaCredentials: build.mutation<
				ButGitea.CredentialCheckResult,
				ButGiteaToken.GiteaAccountIdentifier
			>({
				extraOptions: {
					command: "check_gitea_credentials",
					actionName: "Check Gitea Credentials",
				},
				query: (account) => ({ account }),
			}),
		}),
	});
}

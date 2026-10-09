// forgejo-fork: Forgejo account management, mirroring the Bitbucket service.
import { invalidatesList, providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type {
	ForgejoAccountIdentifier,
	ForgejoAuthStatusResponse,
	ForgejoAuthenticatedUser,
} from "@gitbutler/but-sdk";

export const FORGEJO_USER_SERVICE = new InjectionToken<ForgejoUserService>("ForgejoUserService");

export function isSameForgejoAccountIdentifier(
	a: ForgejoAccountIdentifier,
	b: ForgejoAccountIdentifier,
): boolean {
	return a.host === b.host && a.username === b.username;
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function forgejoAccountIdentifierToString(account: ForgejoAccountIdentifier): string {
	return `${account.host}${UNIT_SEP}${account.username}`;
}

export function stringToForgejoAccountIdentifier(str: string): ForgejoAccountIdentifier | null {
	const [host, username, ...rest] = str.split(UNIT_SEP);
	if (!host || !username || rest.length > 0) return null;
	return { host, username };
}

/** The instance host without its scheme, for compact display. */
export function forgejoHostLabel(account: ForgejoAccountIdentifier): string {
	return account.host.replace(/^https?:\/\//, "");
}

export class ForgejoUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(backendApi: BackendApi) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	get storeForgejoPat() {
		return this.backendApi.endpoints.storeForgejoPat.useMutation();
	}

	get forgetForgejoAccount() {
		return this.backendApi.endpoints.forgetForgejoAccount.useMutation();
	}

	authenticatedUser<T = ForgejoAuthenticatedUser | null>(
		account: ForgejoAccountIdentifier,
		options?: { transform?: (result: ForgejoAuthenticatedUser | null) => T },
	): ReactiveQuery<T> {
		return this.backendApi.endpoints.getForgejoUser.useQuery({ account }, options);
	}

	accounts() {
		return this.backendApi.endpoints.listKnownForgejoAccounts.useQuery();
	}

	deleteAllForgejoAccounts() {
		return this.backendApi.endpoints.clearAllForgejoAccounts.useMutation();
	}
}

function injectBackendEndpoints(api: BackendApi) {
	const invalidatesTags = [
		providesList(ReduxTag.ForgejoUserList),
		invalidatesList(ReduxTag.PullRequests),
		invalidatesList(ReduxTag.Checks),
	];
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetForgejoAccount: build.mutation<void, ForgejoAccountIdentifier>({
				extraOptions: {
					command: "forget_forgejo_account",
					actionName: "Forget Forgejo Account",
				},
				query: (account) => ({ account }),
				invalidatesTags,
			}),
			getForgejoUser: build.query<
				ForgejoAuthenticatedUser | null,
				{ account: ForgejoAccountIdentifier }
			>({
				extraOptions: {
					command: "get_fj_user",
				},
				query: (args) => args,
				providesTags: (_result, _error, { account }) => [
					...providesItem(
						ReduxTag.ForgeUser,
						`forgejo:${forgejoAccountIdentifierToString(account)}`,
					),
				],
			}),
			listKnownForgejoAccounts: build.query<ForgejoAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_forgejo_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.ForgejoUserList)],
			}),
			clearAllForgejoAccounts: build.mutation<void, void>({
				extraOptions: {
					command: "clear_all_forgejo_tokens",
					actionName: "Clear All Forgejo Accounts",
				},
				query: () => ({}),
				invalidatesTags,
			}),
			storeForgejoPat: build.mutation<
				ForgejoAuthStatusResponse,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_forgejo_pat",
					actionName: "Store Forgejo PAT",
				},
				query: (args) => args,
				invalidatesTags,
			}),
		}),
	});
}

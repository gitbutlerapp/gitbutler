import { providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type {
	BitbucketAccountIdentifier,
	BitbucketAuthStatusResponseSensitive,
	BitbucketAuthenticatedUserSensitive,
} from "@gitbutler/but-sdk";

export const BITBUCKET_USER_SERVICE = new InjectionToken<BitbucketUserService>(
	"BitbucketUserService",
);

export function isSameBitbucketAccountIdentifier(
	a: BitbucketAccountIdentifier,
	b: BitbucketAccountIdentifier,
): boolean {
	if (a.type !== b.type) {
		return false;
	}
	switch (a.type) {
		case "apiToken":
			return a.info.email === (b as typeof a).info.email;
	}
}

export type BitbucketAccountIdentifierType = BitbucketAccountIdentifier["type"];

type ExhaustiveBitbucketMap = Record<BitbucketAccountIdentifierType, true>;

const exhaustiveBitbucketMap: ExhaustiveBitbucketMap = {
	apiToken: true,
};

function isBitbucketAccountIdentifierType(text: unknown): text is BitbucketAccountIdentifierType {
	if (typeof text !== "string") {
		return false;
	}
	return exhaustiveBitbucketMap[text as BitbucketAccountIdentifierType] ?? false;
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function bitbucketAccountIdentifierToString(account: BitbucketAccountIdentifier): string {
	switch (account.type) {
		case "apiToken":
			return `${account.type}${UNIT_SEP}${account.info.email}`;
	}
}

export function stringToBitbucketAccountIdentifier(str: string): BitbucketAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length < 2) {
		return null;
	}
	const [type, ...infoParts] = parts;

	if (!isBitbucketAccountIdentifierType(type)) {
		return null;
	}

	switch (type) {
		case "apiToken":
			if (infoParts.length < 1) return null;

			return {
				type: "apiToken",
				info: {
					email: infoParts[0]!,
				},
			};
	}
}

export class BitbucketUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(backendApi: BackendApi) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	get storeBitbucketApiToken() {
		return this.backendApi.endpoints.storeBitbucketApiToken.useMutation();
	}

	get forgetBitbucketAccount() {
		return this.backendApi.endpoints.forgetBitbucketAccount.useMutation();
	}

	authenticatedUser<T = BitbucketAuthenticatedUserSensitive | null>(
		account: BitbucketAccountIdentifier,
		options?: { transform?: (result: BitbucketAuthenticatedUserSensitive | null) => T },
	): ReactiveQuery<T> {
		return this.backendApi.endpoints.getBitbucketUser.useQuery({ account }, options);
	}

	accounts() {
		return this.backendApi.endpoints.listKnownBitbucketAccounts.useQuery();
	}

	deleteAllBitbucketAccounts() {
		return this.backendApi.endpoints.clearAllBitbucketAccounts.useMutation();
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetBitbucketAccount: build.mutation<void, BitbucketAccountIdentifier>({
				extraOptions: {
					command: "forget_bitbucket_account",
					actionName: "Forget Bitbucket Account",
				},
				query: (account) => ({
					account,
				}),
				invalidatesTags: [providesList(ReduxTag.BitbucketUserList)],
			}),
			getBitbucketUser: build.query<
				BitbucketAuthenticatedUserSensitive | null,
				{ account: BitbucketAccountIdentifier }
			>({
				extraOptions: {
					command: "get_bb_user",
				},
				query: (args) => args,
				providesTags: (_result, _error, { account }) => [
					...providesItem(ReduxTag.ForgeUser, `bitbucket:${account.info.email}`),
				],
			}),
			listKnownBitbucketAccounts: build.query<BitbucketAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_bitbucket_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.BitbucketUserList)],
			}),
			clearAllBitbucketAccounts: build.mutation<void, void>({
				extraOptions: {
					command: "clear_all_bitbucket_tokens",
					actionName: "Clear All Bitbucket Accounts",
				},
				query: () => ({}),
				invalidatesTags: [providesList(ReduxTag.BitbucketUserList)],
			}),
			storeBitbucketApiToken: build.mutation<
				BitbucketAuthStatusResponseSensitive,
				{ email: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_bitbucket_api_token",
					actionName: "Store Bitbucket API Token",
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.BitbucketUserList)],
			}),
		}),
	});
}

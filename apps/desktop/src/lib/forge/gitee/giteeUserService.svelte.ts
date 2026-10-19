import { getUserErrorCode } from "$lib/backend/ipc";
import { invalidatesList, providesItem, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { SecretsService } from "$lib/secrets/secretsService";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type {
	GiteeAccountIdentifier,
	GiteeAuthStatusResponse,
	GiteeAuthenticatedUserSensitive,
} from "@gitbutler/but-sdk";

export const GITEE_USER_SERVICE = new InjectionToken<GiteeUserService>("GiteeUserService");

export function giteePatError(error: unknown): string {
	switch (getUserErrorCode(error)) {
		case "GiteeUnauthorized":
			return "The token was not accepted. Check that it is correct and active.";
		case "GiteeForbidden":
			return "Gitee refused access. Check the token scopes and account permissions.";
		default:
			return "Invalid token";
	}
}

export function isSameGiteeAccountIdentifier(
	a: GiteeAccountIdentifier,
	b: GiteeAccountIdentifier,
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

export type GiteeAccountIdentifierType = GiteeAccountIdentifier["type"];

type ExhaustiveGiteeMap = Record<GiteeAccountIdentifierType, true>;

const exhaustiveGiteeMap: ExhaustiveGiteeMap = {
	patUsername: true,
	selfHosted: true,
};

function isGiteeAccountIdentifierType(text: unknown): text is GiteeAccountIdentifierType {
	if (typeof text !== "string") {
		return false;
	}
	return exhaustiveGiteeMap[text as GiteeAccountIdentifierType] ?? false;
}

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = "\u001F";

export function giteeAccountIdentifierToString(account: GiteeAccountIdentifier): string {
	switch (account.type) {
		case "patUsername":
			return `${account.type}${UNIT_SEP}${account.info.username}`;
		case "selfHosted":
			return `${account.type}${UNIT_SEP}${account.info.host}${UNIT_SEP}${account.info.username}`;
	}
}

export function stringToGiteeAccountIdentifier(str: string): GiteeAccountIdentifier | null {
	const parts = str.split(UNIT_SEP);
	if (parts.length < 2) {
		return null;
	}
	const [type, ...infoParts] = parts;

	if (!isGiteeAccountIdentifierType(type)) {
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

export class GiteeUserService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		backendApi: BackendApi,
		private secretsService: SecretsService,
	) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	get storeGiteePat() {
		return this.backendApi.endpoints.storeGiteePat.useMutation();
	}

	get storeGiteeSelfhostedPat() {
		return this.backendApi.endpoints.storeGiteeSelfhostedPat.useMutation();
	}

	get forgetGiteeAccount() {
		return this.backendApi.endpoints.forgetGiteeAccount.useMutation();
	}

	authenticatedUser<T = GiteeAuthenticatedUserSensitive | null>(
		account: GiteeAccountIdentifier,
		options?: { transform?: (result: GiteeAuthenticatedUserSensitive | null) => T },
	): ReactiveQuery<T> {
		return this.backendApi.endpoints.getGiteeUser.useQuery({ account }, options);
	}

	accounts() {
		return this.backendApi.endpoints.listKnownGiteeAccounts.useQuery();
	}
}

export function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetGiteeAccount: build.mutation<void, GiteeAccountIdentifier>({
				extraOptions: {
					command: "forget_gitee_account",
					actionName: "Forget Gitee Account",
				},
				query: (account) => ({
					account,
				}),
				invalidatesTags: [
					providesList(ReduxTag.GiteeUserList),
					invalidatesList(ReduxTag.PullRequests),
					invalidatesList(ReduxTag.Checks),
				],
			}),
			getGiteeUser: build.query<
				GiteeAuthenticatedUserSensitive | null,
				{ account: GiteeAccountIdentifier }
			>({
				extraOptions: {
					command: "get_gitee_user",
				},
				query: (args) => args,
				// The account list tag is what every credential mutation
				// invalidates, so a mounted lookup the old token failed with
				// refetches once a replacement token is stored.
				providesTags: (_result, _error, args) => [
					...providesItem(
						ReduxTag.ForgeUser,
						`gitee:${args.account.type}:${args.account.info.username}`,
					),
					providesList(ReduxTag.GiteeUserList),
				],
			}),
			listKnownGiteeAccounts: build.query<GiteeAccountIdentifier[], void>({
				extraOptions: {
					command: "list_known_gitee_accounts",
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GiteeUserList)],
			}),
			storeGiteePat: build.mutation<GiteeAuthStatusResponse, { accessToken: string }>({
				extraOptions: {
					command: "store_gitee_pat",
					actionName: "Store Gitee PAT",
				},
				query: (args) => args,
				invalidatesTags: invalidatesAfterStoredToken,
			}),
			storeGiteeSelfhostedPat: build.mutation<
				GiteeAuthStatusResponse,
				{ host: string; accessToken: string }
			>({
				extraOptions: {
					command: "store_gitee_selfhosted_pat",
					actionName: "Store Gitee Self-hosted PAT",
				},
				query: (args) => args,
				invalidatesTags: invalidatesAfterStoredToken,
			}),
		}),
	});
}

/**
 * A static tag list is also applied when the mutation fails, which would
 * refetch the user lookup with the unchanged, still-rejected credentials.
 */
function invalidatesAfterStoredToken(_result: unknown, error: unknown) {
	if (error) return [];
	return [
		providesList(ReduxTag.GiteeUserList),
		invalidatesList(ReduxTag.PullRequests),
		invalidatesList(ReduxTag.Checks),
	];
}

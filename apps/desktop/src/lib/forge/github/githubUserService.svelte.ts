import { ghQuery } from '$lib/forge/github/ghQuery';
import { providesItem, providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';
import type { BackendApi, GitHubApi } from '$lib/state/clientState.svelte';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export const GITHUB_USER_SERVICE = new InjectionToken<GitHubUserService>('GitHubUserService');

type IsAuthenticated = RestEndpointMethodTypes['users']['getAuthenticated']['response']['data'];

type Verification = {
	user_code: string;
	device_code: string;
};

type AuthStatusResponse = {
	accessToken: string;
	login: string;
	name: string | null;
	email: string | null;
};

export type AuthenticatedUser = {
	accessToken: string;
	login: string;
	name: string | null;
	email: string | null;
	avatarUrl: string | null;
};

export type GitHubAccountIdentifier =
	| {
			type: 'oAuthUsername';
			info: {
				username: string;
			};
	  }
	| {
			type: 'patUsername';
			info: {
				username: string;
			};
	  }
	| {
			type: 'enterprise';
			info: {
				host: string;
				username: string;
			};
	  };

export class GitHubUserService {
	private api: ReturnType<typeof injectEndpoints>;
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		private backend: IBackend,
		backendApi: BackendApi,
		gitHubApi: GitHubApi
	) {
		this.api = injectEndpoints(gitHubApi);
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	async initDeviceOauth() {
		return await this.backendApi.endpoints.initDeviceOauth.mutate();
	}

	async checkAuthStatus(params: { deviceCode: string }) {
		return await this.backendApi.endpoints.checkAuthStatus.mutate(params);
	}

	get forgetGitHubUsername() {
		return this.backendApi.endpoints.forgetGitHubAccount.useMutation();
	}

	authenticatedUser(account: GitHubAccountIdentifier) {
		return this.backendApi.endpoints.getAccessToken.useQuery({ account });
	}

	accounts() {
		return this.backendApi.endpoints.listKnownGitHubAccounts.useQuery();
	}
	deleteAllGitHubAccounts() {
		return this.backendApi.endpoints.clearAllGitHubAccounts.useMutation();
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getAuthenticated: build.query<IsAuthenticated, void>({
				queryFn: async (_, api) =>
					await ghQuery({
						domain: 'users',
						action: 'getAuthenticated',
						extra: api.extra
					}),
				providesTags: [providesList(ReduxTag.ForgeUser)]
			})
		})
	});
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgetGitHubAccount: build.mutation<void, GitHubAccountIdentifier>({
				extraOptions: {
					command: 'forget_github_account',
					actionName: 'Forget GitHub Username'
				},
				query: (account) => ({
					account
				}),
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)]
			}),
			initDeviceOauth: build.mutation<Verification, void>({
				extraOptions: {
					command: 'init_device_oauth',
					actionName: 'Init GitHub Device OAuth'
				},
				query: () => ({})
			}),
			checkAuthStatus: build.mutation<AuthStatusResponse, { deviceCode: string }>({
				extraOptions: {
					command: 'check_auth_status',
					actionName: 'Check GitHub Auth Status'
				},
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)]
			}),
			getAccessToken: build.query<AuthenticatedUser | null, { account: GitHubAccountIdentifier }>({
				extraOptions: {
					command: 'get_gh_user'
				},
				query: (args) => args,
				providesTags: (_result, _error, username) => [
					...providesItem(ReduxTag.ForgeUser, `github:${username}`)
				]
			}),
			listKnownGitHubAccounts: build.query<GitHubAccountIdentifier[], void>({
				extraOptions: {
					command: 'list_known_github_accounts'
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GitHubUserList)]
			}),
			clearAllGitHubAccounts: build.mutation<void, void>({
				extraOptions: {
					command: 'clear_all_github_tokens',
					actionName: 'Clear All GitHub Accounts'
				},
				query: () => ({}),
				invalidatesTags: [providesList(ReduxTag.GitHubUserList)]
			})
		})
	});
}

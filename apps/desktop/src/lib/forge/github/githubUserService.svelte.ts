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
		return this.backendApi.endpoints.forgetGitHubUsername.useMutation();
	}

	authenticatedUser(username: string) {
		return this.backendApi.endpoints.getAccessToken.useQuery(username);
	}

	usernames() {
		return this.backendApi.endpoints.listKnownGitHubUsernames.useQuery();
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
			forgetGitHubUsername: build.mutation<void, string>({
				extraOptions: {
					command: 'forget_github_username',
					actionName: 'Forget GitHub Username'
				},
				query: (username) => ({
					username
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
			getAccessToken: build.query<AuthenticatedUser | null, string>({
				extraOptions: {
					command: 'get_gh_user'
				},
				query: (username) => ({
					username
				}),
				providesTags: (_result, _error, username) => [
					...providesItem(ReduxTag.ForgeUser, `github:${username}`)
				]
			}),
			listKnownGitHubUsernames: build.query<string[], void>({
				extraOptions: {
					command: 'list_known_github_usernames'
				},
				query: () => ({}),
				providesTags: [providesList(ReduxTag.GitHubUserList)]
			})
		})
	});
}

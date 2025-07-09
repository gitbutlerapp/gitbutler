import { Code, isTauriCommandError } from '$lib/backend/ipc';
import { BaseBranch, type RemoteBranchInfo } from '$lib/baseBranch/baseBranch';
import { showError } from '$lib/notifications/toasts';
import { invalidatesList, invalidatesType, providesType, ReduxTag } from '$lib/state/tags';
import { parseRemoteUrl } from '$lib/url/gitUrl';
import { plainToInstance } from 'class-transformer';
import type { BackendApi } from '$lib/state/clientState.svelte';

function mapBaseBranch(data: unknown): BaseBranch | undefined;
function mapBaseBranch<T>(data: unknown, cb: (baseBranch: BaseBranch) => T): T | undefined;
function mapBaseBranch<T>(
	data: unknown,
	cb?: (baseBranch: BaseBranch) => T
): BaseBranch | T | undefined {
	if (!data) return undefined;
	const baseBranch = plainToInstance(BaseBranch, data);
	if (cb) return cb(baseBranch);
	return baseBranch;
}

export default class BaseBranchService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(private readonly backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	baseBranch(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId
			},
			{
				transform: (data) => {
					return mapBaseBranch(data);
				}
			}
		);
	}

	repo(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId
			},
			{
				transform: (data) =>
					mapBaseBranch(data, (baseBranch) => parseRemoteUrl(baseBranch.remoteUrl))
			}
		);
	}

	pushRepo(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId
			},
			{
				transform: (data) =>
					mapBaseBranch(data, (baseBranch) => parseRemoteUrl(baseBranch.pushRemoteUrl))
			}
		);
	}

	async refreshBaseBranch(projectId: string) {
		await this.api.endpoints.baseBranch.fetch({ projectId }, { forceRefetch: true });
	}

	async fetchFromRemotes(projectId: string, action?: 'auto' | 'modal') {
		return await this.api.endpoints.fetchFromRemotes
			.mutate({ projectId, action })
			.catch((error: unknown) => {
				if (!isTauriCommandError(error)) {
					if (action === 'auto') return;
					showError('Failed to fetch', String(error));
					return;
				}
				const { code } = error;
				if (code === Code.DefaultTargetNotFound) {
					// Swallow this error since user should be taken to project setup page
					return;
				}

				if (code === Code.ProjectsGitAuth) {
					if (action === 'auto') return;
					showError('Failed to authenticate', error.message);
					return;
				}

				if (code === Code.Unknown && error.message?.includes('cargo build -p gitbutler-git')) {
					showError('Run `cargo build -p gitbutler-git`', error.message);
					return;
				}

				if (action !== 'auto') {
					showError('Failed to fetch', error.message);
				}

				console.error(error);
			});
	}

	get setTarget() {
		return this.api.endpoints.setTarget.useMutation();
	}

	get push() {
		return this.api.endpoints.push.useMutation();
	}

	remoteBranches(projectId: string) {
		return this.api.endpoints.remoteBranches.useQuery({ projectId });
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			baseBranch: build.query<unknown, { projectId: string }>({
				extraOptions: { command: 'get_base_branch_data' },
				query: (args) => ({ params: args }),
				providesTags: [providesType(ReduxTag.BaseBranchData)]
			}),
			fetchFromRemotes: build.mutation<void, { projectId: string; action?: string }>({
				extraOptions: { command: 'fetch_from_remotes' },
				query: ({ projectId, action }) => ({
					params: { projectId, action: action ?? 'auto' }
				}),
				invalidatesTags: [
					invalidatesType(ReduxTag.BaseBranchData),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus)
				]
			}),
			setTarget: build.mutation<
				BaseBranch,
				{ projectId: string; branch: string; pushRemote?: string; stashUncommitted?: boolean }
			>({
				extraOptions: { command: 'set_base_branch' },
				query: (args) => ({ params: args }),
				invalidatesTags: [
					invalidatesType(ReduxTag.BaseBranchData),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails)
				]
			}),
			push: build.mutation<void, { projectId: string; withForce?: boolean }>({
				extraOptions: { command: 'push_base_branch' },
				query: (args) => ({ params: args }),
				invalidatesTags: [invalidatesType(ReduxTag.BaseBranchData)]
			}),
			remoteBranches: build.query<RemoteBranchInfo[], { projectId: string }>({
				extraOptions: {
					command: 'git_remote_branches'
				},
				query: (args) => ({ params: args }),
				transformResponse: (data: string[]) => {
					return data
						.map((name) => name.substring(13))
						.sort((a, b) => a.localeCompare(b))
						.map((name) => ({ name }));
				}
			})
		})
	});
}

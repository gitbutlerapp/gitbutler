import { Code } from '$lib/backend/ipc';
import { BaseBranch, type RemoteBranchInfo } from '$lib/baseBranch/baseBranch';
import { showError } from '$lib/notifications/toasts';
import { ReduxTag } from '$lib/state/tags';
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
				transform: (data) => mapBaseBranch(data)
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

	get fetchFromRemotes() {
		return this.api.endpoints.fetchFromRemotes.useMutation({
			onError: (error, { action }) => {
				const { code } = error;
				if (code === Code.DefaultTargetNotFound) {
					// Swallow this error since user should be taken to project setup page
					return;
				}

				if (code === Code.ProjectsGitAuth) {
					showError('Failed to authenticate', error.message);
					return;
				}

				if (action !== undefined) {
					showError('Failed to fetch', error.message);
				}

				console.error(error);
			}
		});
	}

	async refreshRemotes(projectId: string) {
		await this.api.endpoints.fetchFromRemotes.mutate(
			{ projectId },
			{
				onError: (error, { action }) => {
					const { code } = error;
					if (code === Code.DefaultTargetNotFound) {
						// Swallow this error since user should be taken to project setup page
						return;
					}

					if (code === Code.ProjectsGitAuth) {
						showError('Failed to authenticate', error.message);
						return;
					}

					if (action !== undefined) {
						showError('Failed to fetch', error.message);
					}

					console.error(error);
				}
			}
		);
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
				query: ({ projectId }) => ({
					command: 'get_base_branch_data',
					params: { projectId }
				}),
				providesTags: [ReduxTag.BaseBranchData]
			}),
			fetchFromRemotes: build.mutation<void, { projectId: string; action?: string }>({
				query: ({ projectId, action }) => ({
					command: 'fetch_from_remotes',
					params: { projectId, action: action ?? 'auto' }
				}),
				invalidatesTags: [
					ReduxTag.BaseBranchData,
					ReduxTag.Stacks,
					ReduxTag.Commits,
					ReduxTag.UpstreamIntegrationStatus
				],
				transformErrorResponse: (error) => {
					// This is good enough while we check the best way to handle this
					return error.toString();
				}
			}),
			setTarget: build.mutation<
				BaseBranch,
				{ projectId: string; branch: string; pushRemote?: string }
			>({
				query: ({ projectId, branch, pushRemote }) => ({
					command: 'set_base_branch',
					params: { projectId, branch, pushRemote }
				}),
				invalidatesTags: [ReduxTag.BaseBranchData, ReduxTag.Stacks, ReduxTag.Commits]
			}),
			push: build.mutation<void, { projectId: string; withForce?: boolean }>({
				query: ({ projectId, withForce }) => ({
					command: 'push_base_branch',
					params: { projectId, withForce }
				}),
				invalidatesTags: [ReduxTag.BaseBranchData]
			}),
			remoteBranches: build.query<RemoteBranchInfo[], { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'git_remote_branches',
					params: { projectId }
				}),
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

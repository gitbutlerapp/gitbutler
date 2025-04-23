import { gitlab } from '$lib/forge/gitlab/gitlabClient.svelte';
import { detailedMrToInstance, mrToInstance } from '$lib/forge/gitlab/types';
import { providesItem, invalidatesItem, ReduxTag, invalidatesList } from '$lib/state/tags';
import { sleep } from '$lib/utils/sleep';
import { writable } from 'svelte/store';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type {
	CreatePullRequestArgs,
	DetailedPullRequest,
	MergeMethod,
	PullRequest
} from '$lib/forge/interface/types';
import type { QueryOptions } from '$lib/state/butlerModule';
import type { GitLabApi } from '$lib/state/clientState.svelte';
import type { StartQueryActionCreatorOptions } from '@reduxjs/toolkit/query';

export class GitLabPrService implements ForgePrService {
	readonly unit = { name: 'Merge request', abbr: 'MR', symbol: '!' };
	loading = writable(false);
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitlabApi: GitLabApi,
		private posthog?: PostHogWrapper
	) {
		this.api = injectEndpoints(gitlabApi);
	}

	async createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest> {
		this.loading.set(true);

		const request = async () => {
			return await this.api.endpoints.createPr.mutate({
				head: baseBranchName,
				base: upstreamName,
				title,
				body,
				draft
			});
		};

		let attempts = 0;
		let lastError: any;

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				const response = await request();
				this.posthog?.capture('Gitlab MR Successful');
				return response;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		this.posthog?.capture('Gitlab MR Failure');

		throw lastError;
	}

	async fetch(number: number, options?: QueryOptions) {
		const result = $derived(this.api.endpoints.getPr.fetch({ number }, options));
		return await result;
	}

	get(number: number, options?: StartQueryActionCreatorOptions) {
		const result = $derived(this.api.endpoints.getPr.useQuery({ number }, options));
		return result;
	}

	async merge(method: MergeMethod, number: number) {
		await this.api.endpoints.mergePr.mutate({ method, number });
	}

	async reopen(number: number) {
		await this.api.endpoints.updatePr.mutate({
			number,
			update: { state: 'open' }
		});
	}

	async update(
		number: number,
		update: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	) {
		await this.api.endpoints.updatePr.mutate({ number, update });
	}
}

function injectEndpoints(api: GitLabApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, query) => {
					const { api, upstreamProjectId } = gitlab(query.extra);
					const mr = await api.MergeRequests.show(upstreamProjectId, args.number);
					return { data: detailedMrToInstance(mr) };
				},
				providesTags: (_result, _error, args) =>
					providesItem(ReduxTag.GitLabPullRequests, args.number)
			}),
			createPr: build.mutation<
				PullRequest,
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body }, query) => {
					const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
					const upstreamProject = await api.Projects.show(upstreamProjectId);
					const mr = await api.MergeRequests.create(forkProjectId, base, head, title, {
						description: body,
						targetProjectId: upstreamProject.id
					});
					return { data: mrToInstance(mr) };
				},
				invalidatesTags: (result) => [invalidatesItem(ReduxTag.GitLabPullRequests, result?.number)]
			}),
			mergePr: build.mutation<undefined, { number: number; method: MergeMethod }>({
				queryFn: async ({ number }, query) => {
					const { api, upstreamProjectId } = gitlab(query.extra);
					await api.MergeRequests.merge(upstreamProjectId, number);
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GitLabPullRequests)]
			}),
			updatePr: build.mutation<
				void,
				{
					number: number;
					update: {
						targetBase?: string;
						description?: string;
						state?: 'open' | 'closed';
					};
				}
			>({
				queryFn: async ({ number, update }, query) => {
					const { api, upstreamProjectId } = gitlab(query.extra);
					await api.MergeRequests.edit(upstreamProjectId, number, {
						targetBranch: update.targetBase,
						description: update.description
					});
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GitLabPullRequests)]
			})
		})
	});
}

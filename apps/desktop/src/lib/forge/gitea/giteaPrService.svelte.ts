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
import type { GiteaApi } from '$lib/state/clientState.svelte';
import type { StartQueryActionCreatorOptions } from '@reduxjs/toolkit/query';
import { gitea } from '$lib/forge/gitea/giteaClient.svelte';
import {
	detailedPrToInstance,
	prToInstance,
	repoToInstance,
	splitGiteaProjectId,
	userToInstance
} from '$lib/forge/gitea/types';

export class GiteaPrService implements ForgePrService {
	readonly unit = { name: 'Merge request', abbr: 'MR', symbol: '!' };
	loading = writable(false);
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		giteaApi: GiteaApi,
		private posthog?: PostHogWrapper
	) {
		this.api = injectEndpoints(giteaApi);
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
				head: upstreamName,
				base: baseBranchName,
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
				this.posthog?.capture('Gitea PR Successful');
				return response;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		this.posthog?.capture('Gitea PR Failure');

		throw lastError;
	}

	async fetch(number: number, options?: QueryOptions) {
		const result = this.api.endpoints.getPr.fetch({ number }, options);
		return await result;
	}

	get(number: number, options?: StartQueryActionCreatorOptions) {
		return this.api.endpoints.getPr.useQuery({ number }, options);
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

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, query) => {
					const { api, upstreamProjectId } = gitea(query.extra);

					const { owner, repo } = splitGiteaProjectId(upstreamProjectId);

					const repository = repoToInstance(await api.repos.repoGet(owner, repo));

					const pr = detailedPrToInstance(
						await api.repos.repoGetPullRequest(owner, repo, args.number),
						repository.permissions
					);

					const data = {
						...pr,
						repositoryHttpsUrl: repository.httpsUrl,
						repositorySshUrl: repository.sshUrl
					};
					return { data };
				},
				providesTags: (_result, _error, args) =>
					providesItem(ReduxTag.GiteaPullRequests, args.number)
			}),
			createPr: build.mutation<
				PullRequest,
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body }, query) => {
					const { api, upstreamProjectId } = gitea(query.extra);
					const { owner, repo } = splitGiteaProjectId(upstreamProjectId);

					return {
						data: prToInstance(
							await api.repos.repoCreatePullRequest(owner, repo, {
								base,
								body,
								head,
								title
							})
						)
					};
				},
				invalidatesTags: (result) => [invalidatesItem(ReduxTag.GiteaPullRequests, result?.number)]
			}),
			mergePr: build.mutation<undefined, { number: number; method: MergeMethod }>({
				queryFn: async ({ number, method }, query) => {
					const { api, upstreamProjectId } = gitea(query.extra);
					const { owner, repo } = splitGiteaProjectId(upstreamProjectId);
					await api.repos.repoMergePullRequest(owner, repo, number, { Do: method });
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GiteaPullRequests)]
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
					const { api, upstreamProjectId } = gitea(query.extra);
					const { owner, repo } = splitGiteaProjectId(upstreamProjectId);
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GiteaPullRequests)]
			})
		})
	});
}

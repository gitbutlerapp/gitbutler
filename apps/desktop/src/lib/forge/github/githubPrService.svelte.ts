import { ghQuery } from './ghQuery';
import {
	ghResponseToInstance,
	parseGitHubDetailedPullRequest,
	type CreatePrResult,
	type MergeResult,
	type UpdateResult
} from './types';
import {
	MergeMethod,
	type CreatePullRequestArgs,
	type DetailedPullRequest,
	type PullRequest
} from '../interface/types';
import { ReduxTag } from '$lib/state/tags';
import { sleep } from '$lib/utils/sleep';
import { writable } from 'svelte/store';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import type { SubscriptionOptions } from '@reduxjs/toolkit/query';

export class GitHubPrService implements ForgePrService {
	loading = writable(false);
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		githubApi: GitHubApi,
		private posthog?: PostHogWrapper
	) {
		this.api = injectEndpoints(githubApi);
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
			const result = await this.api.endpoints.createPr.useMutation()[0]({
				head: upstreamName,
				base: baseBranchName,
				title,
				body,
				draft
			});
			return ghResponseToInstance(result);
		};

		let attempts = 0;
		let lastError: any;
		let pr: PullRequest | undefined;

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				pr = await request();
				this.posthog?.capture('PR Successful');
				return pr;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		throw lastError;
	}

	async fetch(number: number) {
		const result = await this.api.endpoints.getPr.fetch({ number });
		if (result) {
			return result;
		}
		throw new Error('Invalid response!');
	}

	get(number: number, subscribe?: Reactive<SubscriptionOptions>) {
		const result = $derived.by(() => {
			return this.api.endpoints.getPr.useQuery({ number }, { subscribe });
		});
		return result;
	}

	async merge(method: MergeMethod, number: number) {
		return await this.api.endpoints.mergePr.useMutation()[0]({ method, number });
	}

	async reopen(number: number) {
		return await this.api.endpoints.updatePr.useMutation()[0]({
			number,
			update: { state: 'open' }
		});
	}

	async update(
		number: number,
		update: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	) {
		return await this.api.endpoints.updatePr.useMutation()[0]({ number, update });
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, api) =>
					parseGitHubDetailedPullRequest(
						await ghQuery({
							domain: 'pulls',
							action: 'get',
							parameters: { pull_number: args.number },
							extra: api.extra
						})
					),
				providesTags: [ReduxTag.PullRequests]
			}),
			createPr: build.mutation<
				CreatePrResult,
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body, draft }, api) =>
					await ghQuery({
						domain: 'pulls',
						action: 'create',
						parameters: { head, base, title, body, draft },
						extra: api.extra
					}),
				invalidatesTags: [ReduxTag.PullRequests]
			}),
			mergePr: build.mutation<MergeResult, { number: number; method: MergeMethod }>({
				queryFn: async ({ number, method: method }, api) =>
					await ghQuery({
						domain: 'pulls',
						action: 'merge',
						parameters: { pull_number: number, merge_method: method },
						extra: api.extra
					}),
				invalidatesTags: [ReduxTag.PullRequests]
			}),
			updatePr: build.mutation<
				UpdateResult,
				{
					number: number;
					update: {
						targetBase?: string;
						description?: string;
						state?: 'open' | 'closed';
					};
				}
			>({
				queryFn: async ({ number, update }, api) =>
					await ghQuery({
						domain: 'pulls',
						action: 'update',
						parameters: { pull_number: number, ...update },
						extra: api.extra
					}),
				invalidatesTags: [ReduxTag.PullRequests]
			})
		})
	});
}

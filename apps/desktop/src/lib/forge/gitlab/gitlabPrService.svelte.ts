import { gitlab } from '$lib/forge/gitlab/gitlabClient.svelte';
import { detailedMrToInstance, mrToInstance } from '$lib/forge/gitlab/types';
import { providesItem, invalidatesItem, ReduxTag, invalidatesList } from '$lib/state/tags';
import { sleep } from '$lib/utils/sleep';
import { toSerializable } from '@gitbutler/shared/network/types';
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

function injectEndpoints(api: GitLabApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, query) => {
					try {
						const { api, upstreamProjectId } = gitlab(query.extra);
						const mr = await api.MergeRequests.show(upstreamProjectId, args.number);
						const sourceProject = await api.Projects.show(mr.source_project_id);
						const repositorySshUrl = sourceProject.ssh_url_to_repo;
						const repositoryHttpsUrl = sourceProject.http_url_to_repo;
						const data = {
							...detailedMrToInstance(mr),
							repositoryHttpsUrl,
							repositorySshUrl
						};
						return { data };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: (_result, _error, args) =>
					providesItem(ReduxTag.GitLabPullRequests, args.number)
			}),
			createPr: build.mutation<
				PullRequest,
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body, draft }, query) => {
					try {
						const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
						const upstreamProject = await api.Projects.show(upstreamProjectId);

						// GitLab uses title prefix to mark drafts: "Draft:", "[Draft]", or "(Draft)"
						const finalTitle = draft ? `[Draft] ${title}` : title;

						const mr = await api.MergeRequests.create(forkProjectId, head, base, finalTitle, {
							description: body,
							targetProjectId: upstreamProject.id,
							removeSourceBranch: true
						});
						return { data: mrToInstance(mr) };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				invalidatesTags: (result) => [invalidatesItem(ReduxTag.GitLabPullRequests, result?.number)]
			}),
			mergePr: build.mutation<undefined, { number: number; method: MergeMethod }>({
				queryFn: async ({ number, method }, query) => {
					try {
						const { api, upstreamProjectId } = gitlab(query.extra);

						// Note: Unlike GitHub, GitLab's rebase is a two-step async process
						if (method === 'rebase') {
							// Rebase the source branch onto the target branch
							// This is an async operation that returns immediately with 202 status
							await api.MergeRequests.rebase(upstreamProjectId, number, {
								skipCI: false
							});

							// Poll for rebase completion before merging
							// GitLab's rebase operation is asynchronous, so we need to wait
							const maxAttempts = 30; // 30 seconds timeout
							let attempt = 0;

							while (attempt < maxAttempts) {
								// Check rebase status immediately (first iteration) or after waiting
								const mr = await api.MergeRequests.show(upstreamProjectId, number, {
									includeRebaseInProgress: true
								});

								// Check if rebase completed successfully
								if (!mr.rebase_in_progress) {
									// Check for rebase errors
									if (mr.merge_error) {
										throw new Error(`Rebase failed: ${mr.merge_error}`);
									}
									break;
								}

								attempt++;
								if (attempt >= maxAttempts) {
									throw new Error('Rebase operation timed out. Please try merging again later.');
								}

								await sleep(1000);
							}

							// After rebase completes successfully, perform the merge
							await api.MergeRequests.merge(upstreamProjectId, number, {
								shouldRemoveSourceBranch: true
							});
						} else {
							// For 'merge' and 'squash' methods, use the merge API directly
							await api.MergeRequests.merge(upstreamProjectId, number, {
								squash: method === 'squash',
								shouldRemoveSourceBranch: true
							});
						}

						return { data: undefined };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
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
					try {
						const { api, upstreamProjectId } = gitlab(query.extra);
						await api.MergeRequests.edit(upstreamProjectId, number, {
							targetBranch: update.targetBase,
							description: update.description
						});
						return { data: undefined };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				invalidatesTags: [invalidatesList(ReduxTag.GitLabPullRequests)]
			})
		})
	});
}

import { gitea } from "$lib/forge/gitea/giteaClient.svelte";
import { giteaResponseToDetailedPullRequest, giteaResponseToPullRequest } from "$lib/forge/gitea/types";
import {
	MergeMethod,
	type CreatePullRequestArgs,
	type DetailedPullRequest,
	type PullRequest,
} from "$lib/forge/interface/types";
import { providesItem, invalidatesItem, ReduxTag, invalidatesList } from "$lib/state/tags";
import { sleep } from "$lib/utils/sleep";
import { writable } from "svelte/store";
import type { ForgePrService } from "$lib/forge/interface/forgePrService";
import type { QueryOptions } from "$lib/state/butlerModule";
import type { BackendApi, GiteaApi } from "$lib/state/clientState.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { StartQueryActionCreatorOptions } from "@reduxjs/toolkit/query";

export class GiteaPrService implements ForgePrService {
	readonly unit = { name: "Pull request", abbr: "PR", symbol: "#" };
	loading = writable(false);
	private api: ReturnType<typeof injectEndpoints>;
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		giteaApi: GiteaApi,
		backendApi: BackendApi,
		private posthog?: PostHogWrapper,
	) {
		this.api = injectEndpoints(giteaApi);
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	async createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName,
	}: CreatePullRequestArgs): Promise<PullRequest> {
		this.loading.set(true);
		const request = async () => {
			return giteaResponseToPullRequest(
				await this.api.endpoints.createPr.mutate({
					head: upstreamName,
					base: baseBranchName,
					title,
					body,
					draft,
				}),
			);
		};

		let attempts = 0;
		let lastError: any;
		let pr: PullRequest | undefined;

		while (attempts < 4) {
			try {
				pr = await request();
				this.posthog?.capture("Gitea PR Successful");
				return pr;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		this.posthog?.capture("Gitea PR Failure");
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
			update: { state: "open" },
		});
	}

	async update(
		number: number,
		update: { description?: string; state?: "open" | "closed"; targetBase?: string },
	) {
		await this.api.endpoints.updatePr.mutate({ number, update });
	}

	async setDraft(projectId: string, reviewId: number, draft: boolean) {
		await this.backendApi.endpoints.setDraftGT.mutate({ projectId, reviewId, draft });
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			setDraftGT: build.mutation<void, { projectId: string; reviewId: number; draft: boolean }>({
				extraOptions: { command: "set_review_draftiness" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.GiteaPRs, reviewId),
				],
			}),
		}),
	});
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, api) => {
					const client = gitea(api.extra);
					const response = await client.fetch(`/repos/${client.owner}/${client.repo}/pulls/${args.number}`);
					if (!response.ok) {
						return { error: { name: "Gitea API error", message: `Failed to fetch PR ${args.number}: ${response.status}` } };
					}
					const prData = await response.json();
					return { data: giteaResponseToDetailedPullRequest(prData) };
				},
				providesTags: (_result, _error, args) => providesItem(ReduxTag.GiteaPRs, args.number),
			}),
			createPr: build.mutation<
				any, // Gitea response data
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body, draft }, api) => {
					const client = gitea(api.extra);
					const response = await client.fetch(`/repos/${client.owner}/${client.repo}/pulls`, {
						method: "POST",
						body: JSON.stringify({ head, base, title, body, draft }),
					});
					if (!response.ok) {
						return { error: { name: "Gitea API error", message: `Failed to create PR: ${response.status}` } };
					}
					const prData = await response.json();
					return { data: prData };
				},
				invalidatesTags: (result) => [invalidatesItem(ReduxTag.GiteaPRs, result?.number)],
			}),
			mergePr: build.mutation<void, { number: number; method: MergeMethod }>({
				queryFn: async ({ number, method }, api) => {
					const client = gitea(api.extra);
					const response = await client.fetch(`/repos/${client.owner}/${client.repo}/pulls/${number}/merge`, {
						method: "POST",
						body: JSON.stringify({
							Do: method === MergeMethod.Squash ? "squash" : method === MergeMethod.Rebase ? "rebase" : "merge",
						}),
					});
					if (!response.ok) {
						return { error: { name: "Gitea API error", message: `Failed to merge PR ${number}: ${response.status}` } };
					}
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GiteaPRs)],
			}),
			updatePr: build.mutation<
				void,
				{
					number: number;
					update: {
						targetBase?: string;
						description?: string;
						state?: "open" | "closed";
					};
				}
			>({
				queryFn: async ({ number, update }, api) => {
					const client = gitea(api.extra);
					const response = await client.fetch(`/repos/${client.owner}/${client.repo}/pulls/${number}`, {
						method: "PATCH",
						body: JSON.stringify({
							base: update.targetBase,
							body: update.description,
							state: update.state,
						}),
					});
					if (!response.ok) {
						return { error: { name: "Gitea API error", message: `Failed to update PR ${number}: ${response.status}` } };
					}
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.GiteaPRs)],
			}),
		}),
	});
}

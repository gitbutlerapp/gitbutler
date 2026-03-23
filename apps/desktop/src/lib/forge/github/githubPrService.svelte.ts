import { ghQuery } from "$lib/forge/github/ghQuery";
import {
	ghResponseToInstance,
	parseGitHubDetailedPullRequest,
	type CreatePrResult,
	type DetailedGitHubPullRequestWithPermissions,
	type GitHubRepoPermissions,
} from "$lib/forge/github/types";
import {
	MergeMethod,
	type CreatePullRequestArgs,
	type DetailedPullRequest,
	type PullRequest,
} from "$lib/forge/interface/types";
import { eventualConsistencyCheck } from "$lib/forge/shared/progressivePolling";
import { providesItem, invalidatesItem, ReduxTag, invalidatesList } from "$lib/state/tags";
import { sleep } from "$lib/utils/sleep";
import { writable } from "svelte/store";
import type { ForgePrService } from "$lib/forge/interface/forgePrService";
import type { QueryOptions } from "$lib/state/butlerModule";
import type { BackendApi, GitHubApi } from "$lib/state/clientState.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { StartQueryActionCreatorOptions } from "@reduxjs/toolkit/query";

export class GitHubPrService implements ForgePrService {
	readonly unit = { name: "Pull request", abbr: "PR", symbol: "#" };
	loading = writable(false);
	private api: ReturnType<typeof injectEndpoints>;
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		githubApi: GitHubApi,
		backendApi: BackendApi,
		private posthog?: PostHogWrapper,
	) {
		this.api = injectEndpoints(githubApi);
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
			return ghResponseToInstance(
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

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				pr = await request();
				this.posthog?.capture("PR Successful");
				return pr;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		this.posthog?.capture("PR Failure");
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
		await this.backendApi.endpoints.setDraft.mutate({ projectId, reviewId, draft });
	}
}

async function fetchRepoPermissions(
	owner: string,
	repo: string,
	extra: unknown,
): Promise<GitHubRepoPermissions | undefined> {
	try {
		const repoResponse = await ghQuery(
			{
				domain: "repos",
				action: "get",
				parameters: { owner, repo },
				extra: extra,
			},
			extra,
			"required",
		);

		if (repoResponse.error) {
			throw repoResponse.error;
		}

		return repoResponse.data.permissions;
	} catch (err) {
		console.error(`Exception fetching repository permissions for ${owner}/${repo}:`, err);
		return undefined;
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			setAutoMerge: build.mutation<void, { projectId: string; reviewId: number; enable: boolean }>({
				extraOptions: { command: "set_review_auto_merge" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
			setDraft: build.mutation<void, { projectId: string; reviewId: number; draft: boolean }>({
				extraOptions: { command: "set_review_draftiness" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
		}),
	});
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getPr: build.query<DetailedPullRequest, { number: number }>({
				queryFn: async (args, api) => {
					async function getPrByNumber() {
						return await ghQuery({
							domain: "pulls",
							action: "get",
							parameters: { pull_number: args.number },
							extra: api.extra,
						});
					}

					const prResponse = await eventualConsistencyCheck(getPrByNumber, (response) => {
						if (response.error) {
							// Stop if there's an error
							return true;
						}
						// Stop if we have a valid response
						return response.data?.updated_at !== undefined;
					});

					if (prResponse.error) {
						return { error: prResponse.error };
					}

					const prData = prResponse.data;
					const owner = prData.base?.repo?.owner?.login;
					const repo = prData.base?.repo?.name;

					const permissions =
						owner && repo ? await fetchRepoPermissions(owner, repo, api.extra) : undefined;

					const combinedData: DetailedGitHubPullRequestWithPermissions = {
						...prData,
						permissions,
					};

					const finalResult = parseGitHubDetailedPullRequest({ data: combinedData });

					if (finalResult.error) {
						return { error: finalResult.error };
					}

					return { data: finalResult.data };
				},
				providesTags: (_result, _error, args) => providesItem(ReduxTag.PullRequests, args.number),
			}),
			createPr: build.mutation<
				CreatePrResult,
				{ head: string; base: string; title: string; body: string; draft: boolean }
			>({
				queryFn: async ({ head, base, title, body, draft }, api) =>
					await ghQuery({
						domain: "pulls",
						action: "create",
						parameters: { head, base, title, body, draft },
						extra: api.extra,
					}),
				invalidatesTags: (result) => [invalidatesItem(ReduxTag.PullRequests, result?.number)],
			}),
			mergePr: build.mutation<void, { number: number; method: MergeMethod }>({
				queryFn: async ({ number, method: method }, api) => {
					const result = await ghQuery({
						domain: "pulls",
						action: "merge",
						parameters: { pull_number: number, merge_method: method },
						extra: api.extra,
					});

					if (result.error) {
						return { error: result.error };
					}

					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.PullRequests)],
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
					const result = await ghQuery({
						domain: "pulls",
						action: "update",
						parameters: {
							pull_number: number,
							base: update.targetBase,
							body: update.description,
							state: update.state,
						},
						extra: api.extra,
					});
					if (result.error) {
						return { error: result.error };
					}
					return { data: undefined };
				},
				invalidatesTags: [invalidatesList(ReduxTag.PullRequests)],
			}),
		}),
	});
}

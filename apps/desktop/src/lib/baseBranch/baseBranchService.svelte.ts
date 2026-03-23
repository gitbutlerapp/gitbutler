import { BaseBranch, type ForgeProvider, type RemoteBranchInfo } from "$lib/baseBranch/baseBranch";
import { Code } from "$lib/error/knownErrors";
import { isReduxError } from "$lib/error/reduxError";
import { showError } from "$lib/error/showError";
import { parseRemoteUrl } from "$lib/git/gitUrl";
import { invalidatesList, invalidatesType, providesType, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import { plainToInstance } from "class-transformer";
import type { BackendApi } from "$lib/state/clientState.svelte";

function mapBaseBranch(data: unknown): BaseBranch | undefined;
function mapBaseBranch<T>(data: unknown, cb: (baseBranch: BaseBranch) => T): T | undefined;
function mapBaseBranch<T>(
	data: unknown,
	cb?: (baseBranch: BaseBranch) => T,
): BaseBranch | T | undefined {
	if (!data) return undefined;
	const baseBranch = plainToInstance(BaseBranch, data);
	if (cb) return cb(baseBranch);
	return baseBranch;
}

export const BASE_BRANCH_SERVICE = new InjectionToken<BaseBranchService>("BaseBranchService");

export default class BaseBranchService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(readonly backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	forgeProvider(projectId: string) {
		return this.api.endpoints.forgeProvider.useQuery({ projectId });
	}

	baseBranch(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) => {
					return mapBaseBranch(data);
				},
			},
		);
	}

	baseBranchShortName(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) =>
					mapBaseBranch(data, (baseBranch) => {
						if (baseBranch.branchName.startsWith(baseBranch.remoteName + "/")) {
							return baseBranch.branchName.substring(baseBranch.remoteName.length + 1);
						}
						return baseBranch.branchName;
					}),
			},
		);
	}

	repo(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) =>
					mapBaseBranch(data, (baseBranch) => parseRemoteUrl(baseBranch.remoteUrl)),
			},
		);
	}

	pushRepo(projectId: string) {
		return this.api.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) =>
					mapBaseBranch(data, (baseBranch) => parseRemoteUrl(baseBranch.pushRemoteUrl)),
			},
		);
	}

	async refreshBaseBranch(projectId: string) {
		await this.api.endpoints.baseBranch.fetch({ projectId }, { forceRefetch: true });
	}

	async fetchFromRemotes(projectId: string, action?: "auto" | "modal") {
		return await this.api.endpoints.fetchFromRemotes
			.mutate({ projectId, action })
			.catch((error: unknown) => {
				if (!isReduxError(error)) {
					if (action === "auto") return;
					showError("Failed to fetch", String(error));
					return;
				}
				const { code } = error;
				if (code === Code.DefaultTargetNotFound) {
					// Swallow this error since user should be taken to project setup page
					return;
				}

				if (code === Code.ProjectsGitAuth) {
					if (action === "auto") return;
					showError("Failed to authenticate", error.message);
					return;
				}

				if (code === Code.Unknown && error.message?.includes("cargo build -p gitbutler-git")) {
					showError("Run `cargo build -p gitbutler-git`", error.message);
					return;
				}

				if (action !== "auto") {
					showError("Failed to fetch", error.message);
				}

				console.error(error);
			});
	}

	get setTarget() {
		return this.api.endpoints.setTarget.useMutation();
	}

	get switchBackToWorkspace() {
		return this.api.endpoints.switchBackToWorkspace.useMutation();
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
			forgeProvider: build.query<ForgeProvider | null, { projectId: string }>({
				extraOptions: { command: "forge_provider" },
				query: (args) => args,
				providesTags: [providesType(ReduxTag.ForgeProvider)],
			}),
			baseBranch: build.query<unknown, { projectId: string }>({
				extraOptions: { command: "get_base_branch_data" },
				query: (args) => args,
				providesTags: [providesType(ReduxTag.BaseBranchData)],
			}),
			fetchFromRemotes: build.mutation<void, { projectId: string; action?: string }>({
				extraOptions: { command: "fetch_from_remotes" },
				query: ({ projectId, action }) => ({
					projectId,
					action: action ?? "auto",
				}),
				invalidatesTags: [
					// No need to invalidate base branch, we should be listening
					// for all FETCH events, and refreshing manually.
					invalidatesList(ReduxTag.Stacks), // Probably this is still needed??
					invalidatesList(ReduxTag.StackDetails), // Probably this is still needed??
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
				],
			}),
			setTarget: build.mutation<
				BaseBranch,
				{ projectId: string; branch: string; pushRemote?: string; stashUncommitted?: boolean }
			>({
				extraOptions: { command: "set_base_branch" },
				query: (args) => args,
				invalidatesTags: [
					invalidatesType(ReduxTag.ForgeProvider),
					invalidatesType(ReduxTag.BaseBranchData),
					invalidatesList(ReduxTag.Stacks), // Probably this is still needed??
					invalidatesList(ReduxTag.StackDetails), // Probably this is still needed??
				],
			}),
			switchBackToWorkspace: build.mutation<BaseBranch, { projectId: string }>({
				extraOptions: { command: "switch_back_to_workspace" },
				query: (args) => args,
				invalidatesTags: [
					invalidatesType(ReduxTag.ForgeProvider),
					invalidatesType(ReduxTag.BaseBranchData),
					invalidatesList(ReduxTag.Stacks), // Probably this is still needed??
					invalidatesList(ReduxTag.StackDetails), // Probably this is still needed??
				],
			}),
			push: build.mutation<void, { projectId: string; withForce?: boolean }>({
				extraOptions: { command: "push_base_branch" },
				query: (args) => args,
				invalidatesTags: [invalidatesType(ReduxTag.BaseBranchData)],
			}),
			remoteBranches: build.query<RemoteBranchInfo[], { projectId: string }>({
				extraOptions: { command: "git_remote_branches" },
				query: (args) => args,
				transformResponse: (data: string[]) => {
					return data
						.map((name) => name.substring(13))
						.sort((a, b) => a.localeCompare(b))
						.map((name) => ({ name }));
				},
			}),
		}),
	});
}

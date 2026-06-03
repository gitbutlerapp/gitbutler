import { showError } from "$lib/error/showError";
import { parseRemoteUrl } from "$lib/git/gitUrl";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { BaseBranch } from "@gitbutler/but-sdk";

export const BASE_BRANCH_SERVICE = new InjectionToken<BaseBranchService>("BaseBranchService");

export default class BaseBranchService {
	constructor(private backendApi: BackendApi) {}

	forgeProvider(projectId: string) {
		return this.backendApi.endpoints.forgeProvider.useQuery({ projectId });
	}

	baseBranch(projectId: string) {
		return this.backendApi.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) => data as BaseBranch | undefined,
			},
		);
	}

	baseBranchShortName(projectId: string) {
		return this.backendApi.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) => {
					return data?.shortName;
				},
			},
		);
	}

	repo(projectId: string) {
		return this.backendApi.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) => {
					return data ? parseRemoteUrl(data.remoteUrl) : undefined;
				},
			},
		);
	}

	pushRepo(projectId: string) {
		return this.backendApi.endpoints.baseBranch.useQuery(
			{
				projectId,
			},
			{
				transform: (data) => {
					const baseBranch = data as BaseBranch | undefined;
					return baseBranch ? parseRemoteUrl(baseBranch.pushRemoteUrl) : undefined;
				},
			},
		);
	}

	async refreshBaseBranch(projectId: string) {
		await this.backendApi.endpoints.baseBranch.fetch({ projectId }, { forceRefetch: true });
	}

	async fetchFromRemotes(projectId: string, action?: "auto" | "modal") {
		return await this.backendApi.endpoints.fetchFromRemotes
			.mutate({ projectId, action })
			.catch((error: unknown) => {
				// Auto-fetches run on a timer and shouldn't surface to the user.
				// `showError` defers per-code presentation (silent for
				// `DefaultTargetNotFound`, warning for `ProjectGitAuth`, the
				// cargo-build hint for the `Unknown` + cargo message) to the
				// classifier.
				if (action === "auto") return;
				showError("Failed to fetch", error);
			});
	}

	get setTarget() {
		return this.backendApi.endpoints.setTarget.useMutation();
	}

	get switchBackToWorkspace() {
		return this.backendApi.endpoints.switchBackToWorkspace.useMutation();
	}

	get push() {
		return this.backendApi.endpoints.pushBaseBranch.useMutation();
	}

	remoteBranches(projectId: string) {
		return this.backendApi.endpoints.remoteBranches.useQuery({ projectId });
	}
}

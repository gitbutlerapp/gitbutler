import { Code } from "$lib/error/knownErrors";
import { isReduxError } from "$lib/error/reduxError";
import { showError } from "$lib/error/showError";
import { parseRemoteUrl } from "$lib/git/gitUrl";
import { InjectionToken } from "@gitbutler/core/context";
import type { BaseBranch } from "$lib/baseBranch/baseBranch";
import type { BackendApi } from "$lib/state/clientState.svelte";

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

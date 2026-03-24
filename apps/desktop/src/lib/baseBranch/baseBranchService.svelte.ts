import { BaseBranch } from "$lib/baseBranch/baseBranch";
import { Code } from "$lib/error/knownErrors";
import { isReduxError } from "$lib/error/reduxError";
import { showError } from "$lib/error/showError";
import { parseRemoteUrl } from "$lib/git/gitUrl";
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
				transform: (data) => {
					return mapBaseBranch(data);
				},
			},
		);
	}

	baseBranchShortName(projectId: string) {
		return this.backendApi.endpoints.baseBranch.useQuery(
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
		return this.backendApi.endpoints.baseBranch.useQuery(
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
		return this.backendApi.endpoints.baseBranch.useQuery(
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

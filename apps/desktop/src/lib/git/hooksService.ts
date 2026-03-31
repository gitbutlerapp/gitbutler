import { InjectionToken } from "@gitbutler/core/context";
import { chipToasts } from "@gitbutler/ui";
import type { BackendApi } from "$lib/state/backendApi";
import type { DiffSpec } from "@gitbutler/but-sdk";

export const HOOKS_SERVICE = new InjectionToken<HooksService>("HooksService");

export class HooksService {
	constructor(private backendApi: BackendApi) {}

	get message() {
		return this.backendApi.endpoints.messageHook.useMutation();
	}

	// Promise-based wrapper methods with toast handling
	async runPreCommitHooks(projectId: string, changes: DiffSpec[]): Promise<void> {
		const loadingToastId = chipToasts.loading("Started pre-commit hooks");

		try {
			const result = await this.backendApi.endpoints.preCommitDiffspecs.mutate({
				projectId,
				changes,
			});

			if (result?.status === "failure") {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(formatError(result.error));
			}

			chipToasts.removeChipToast(loadingToastId);
			chipToasts.success("Pre-commit hooks succeeded");
		} catch (e: unknown) {
			chipToasts.removeChipToast(loadingToastId);
			throw e;
		}
	}

	async runPostCommitHooks(projectId: string): Promise<void> {
		const loadingToastId = chipToasts.loading("Started post-commit hooks");

		try {
			const result = await this.backendApi.endpoints.postCommit.mutate({
				projectId,
			});

			if (result?.status === "failure") {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(formatError(result.error));
			}

			chipToasts.removeChipToast(loadingToastId);
			chipToasts.success("Post-commit hooks succeeded");
		} catch (e: unknown) {
			chipToasts.removeChipToast(loadingToastId);
			throw e;
		}
	}
}

function formatError(error: string): string {
	return `${error}\n\nIf you don't want git hooks to run, disable "Run Git hooks" in project settings.`;
}

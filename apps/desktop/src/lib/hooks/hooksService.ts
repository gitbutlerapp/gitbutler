import { InjectionToken } from '@gitbutler/core/context';
import { chipToasts } from '@gitbutler/ui';
import type { DiffSpec } from '$lib/hunks/hunk';
import type { BackendApi } from '$lib/state/clientState.svelte';

export type HookStatus =
	| {
			status: 'success';
	  }
	| {
			status: 'notconfigured';
	  }
	| {
			status: 'failure';
			error: string;
	  };

export type MessageHookStatus =
	| {
			status: 'success';
	  }
	| {
			status: 'message';
			message: string;
	  }
	| {
			status: 'notconfigured';
	  }
	| {
			status: 'failure';
			error: string;
	  };

export const HOOKS_SERVICE = new InjectionToken<HooksService>('HooksService');

export class HooksService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get message() {
		return this.api.endpoints.message.useMutation();
	}

	// Promise-based wrapper methods with toast handling
	async runPreCommitHooks(projectId: string, changes: DiffSpec[]): Promise<void> {
		const loadingToastId = chipToasts.loading('Started pre-commit hooks');

		try {
			const result = await this.api.endpoints.preCommitDiffspecs.mutate({
				projectId,
				changes
			});

			if (result?.status === 'failure') {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(formatError(result.error));
			}

			chipToasts.removeChipToast(loadingToastId);
			chipToasts.success('Pre-commit hooks succeeded');
		} catch (e: unknown) {
			chipToasts.removeChipToast(loadingToastId);
			throw e;
		}
	}

	async runPostCommitHooks(projectId: string): Promise<void> {
		const loadingToastId = chipToasts.loading('Started post-commit hooks');

		try {
			const result = await this.api.endpoints.postCommit.mutate({
				projectId
			});

			if (result?.status === 'failure') {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(formatError(result.error));
			}

			chipToasts.removeChipToast(loadingToastId);
			chipToasts.success('Post-commit hooks succeeded');
		} catch (e: unknown) {
			chipToasts.removeChipToast(loadingToastId);
			throw e;
		}
	}
}

function formatError(error: string): string {
	return `${error}\n\nIf you don't want git hooks to be run, you can disable them in the project settings.`;
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			preCommitDiffspecs: build.mutation<HookStatus, { projectId: string; changes: DiffSpec[] }>({
				extraOptions: { command: 'pre_commit_hook_diffspecs' },
				query: (args) => args
			}),
			postCommit: build.mutation<HookStatus, { projectId: string }>({
				extraOptions: { command: 'post_commit_hook' },
				query: (args) => args
			}),
			message: build.mutation<MessageHookStatus, { projectId: string; message: string }>({
				extraOptions: { command: 'message_hook' },
				query: (args) => args
			})
		})
	});
}

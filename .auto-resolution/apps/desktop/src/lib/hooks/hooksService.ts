import { InjectionToken } from '@gitbutler/shared/context';
import { chipToasts } from '@gitbutler/ui';
import type { IBackend } from '$lib/backend';
import type { DiffSpec } from '$lib/hunks/hunk';

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
	constructor(private backend: IBackend) {}

	async preCommitDiffspecs(projectId: string, changes: DiffSpec[]) {
		return await this.backend.invoke<HookStatus>('pre_commit_hook_diffspecs', {
			projectId,
			changes
		});
	}

	async postCommit(projectId: string) {
		return await this.backend.invoke<HookStatus>('post_commit_hook', {
			projectId
		});
	}

	async message(projectId: string, message: string) {
		return await this.backend.invoke<MessageHookStatus>('message_hook', {
			projectId,
			message
		});
	}

	async runPreCommitHooks(projectId: string, changes: DiffSpec[]): Promise<void> {
		const loadingToastId = chipToasts.loading('Started pre-commit hooks');

		try {
			const result = await this.preCommitDiffspecs(projectId, changes);

			if (result?.status === 'failure') {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(result.error);
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
			const result = await this.postCommit(projectId);

			if (result?.status === 'failure') {
				chipToasts.removeChipToast(loadingToastId);
				throw new Error(result.error);
			}

			chipToasts.removeChipToast(loadingToastId);
			chipToasts.success('Post-commit hooks succeeded');
		} catch (e: unknown) {
			chipToasts.removeChipToast(loadingToastId);
			throw e;
		}
	}
}

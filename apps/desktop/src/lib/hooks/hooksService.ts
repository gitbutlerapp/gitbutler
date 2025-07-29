import { InjectionToken } from '@gitbutler/shared/context';
import { chipToasts } from '@gitbutler/ui';
import type { Tauri } from '$lib/backend/tauri';
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

class HookError extends Error {}

export const HOOKS_SERVICE = new InjectionToken<HooksService>('HooksService');

export class HooksService {
	constructor(private tauri: Tauri) {}

	async preCommitDiffspecs(projectId: string, changes: DiffSpec[]) {
		return await this.tauri.invoke<HookStatus>('pre_commit_hook_diffspecs', {
			projectId,
			changes
		});
	}

	async postCommit(projectId: string) {
		return await this.tauri.invoke<HookStatus>('post_commit_hook', {
			projectId
		});
	}

	async message(projectId: string, message: string) {
		return await this.tauri.invoke<MessageHookStatus>('message_hook', {
			projectId,
			message
		});
	}

	async runPreCommitHooks(projectId: string, changes: DiffSpec[]): Promise<boolean> {
		let failed = false;
		try {
			await chipToasts.promise(
				(async () => {
					const result = await this.preCommitDiffspecs(projectId, changes);
					if (result?.status === 'failure') {
						failed = true;
						console.error('Pre-commit hooks failed:', result.error);
						throw new HookError(result.error);
					}
				})(),
				{
					loading: 'Started pre-commit hooks',
					success: 'Pre-commit hooks succeded',
					error: 'Pre-commit hooks failed'
				}
			);
		} catch (e: unknown) {
			if (!(e instanceof HookError)) {
				throw e;
			}
		}

		return failed;
	}

	async runPostCommitHooks(projectId: string): Promise<boolean> {
		let failed = false;
		try {
			await chipToasts.promise(
				(async () => {
					const result = await this.postCommit(projectId);
					if (result?.status === 'failure') {
						failed = true;
						console.error('Post-commit hooks failed:', result.error);
						throw new HookError(result.error);
					}
				})(),
				{
					loading: 'Started post-commit hooks',
					success: 'Post-commit hooks succeded',
					error: 'Post-commit hooks failed'
				}
			);
		} catch (e) {
			if (!(e instanceof HookError)) {
				throw e;
			}
		}

		return failed;
	}
}

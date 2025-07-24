import { InjectionToken } from '@gitbutler/shared/context';
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
}

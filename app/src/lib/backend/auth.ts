import { invoke } from '$lib/backend/ipc';

export type GitCredentialCheck = {
	error?: string;
	name?: string;
	ok: boolean;
};

export type CredentialCheckError = {
	check: string;
	message: string;
};

export class AuthService {
	constructor() {}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined) {
		if (!remoteName) return;
		const resp = await invoke<string>('git_test_fetch', {
			projectId: projectId,
			action: 'modal',
			remoteName
		});
		// fix: we should have a response with an optional error
		if (resp) throw new Error(resp);
		return;
	}

	async checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined
	) {
		if (!remoteName) return;
		const resp = await invoke<string>('git_test_push', {
			projectId: projectId,
			action: 'modal',
			remoteName,
			branchName
		});
		if (resp) throw new Error(resp);
		return { name: 'push', ok: true };
	}

	async getPublicKey() {
		return await invoke<string>('get_public_key');
	}
}

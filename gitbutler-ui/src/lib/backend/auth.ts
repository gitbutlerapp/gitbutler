import { invoke } from './ipc';

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
	async checkGitFetch(projectId: string, remoteName: string) {
		const resp = await invoke<string>('git_test_fetch', {
			projectId: projectId,
			remoteName
		});
		if (resp) throw new Error(resp);
		return;
	}

	async checkGitPush(projectId: string, remoteName: string, branchName: string) {
		const resp = await invoke<string>('git_test_push', {
			projectId: projectId,
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

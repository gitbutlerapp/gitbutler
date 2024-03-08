import { invoke } from './ipc';

export type GitCredentialCheck = {
	error?: string;
	ok: boolean;
};

export class AuthService {
	async checkGitFetch(projectId: string, remoteName: string | null | undefined) {
		if (!remoteName) return { ok: false, error: 'No remote specified' };
		try {
			const resp = await invoke<string>('git_test_fetch', {
				projectId: projectId,
				remoteName
			});
			return { ok: !resp };
		} catch (err: any) {
			return { ok: false, error: err.message };
		}
	}

	async checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined
	) {
		if (!remoteName) return { ok: false, error: 'No remote specified' };
		if (!branchName) return { ok: false, error: 'No branchspecified' };
		try {
			const resp = await invoke<string>('git_test_push', {
				projectId: projectId,
				remoteName,
				branchName
			});
			return { ok: !resp };
		} catch (err: any) {
			return { ok: false, error: err.message };
		}
	}

	async getPublicKey() {
		return await invoke<string>('get_public_key');
	}
}

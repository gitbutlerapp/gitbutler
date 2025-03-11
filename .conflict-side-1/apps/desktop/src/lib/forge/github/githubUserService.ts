import { buildContextStore } from '@gitbutler/shared/context';
import type { Tauri } from '$lib/backend/tauri';
import type { Octokit } from '@octokit/rest';

type Verification = {
	user_code: string;
	device_code: string;
};

export class GitHubUserService {
	constructor(private octokit: Octokit) {}

	async fetchGitHubLogin(): Promise<string> {
		try {
			const rsp = await this.octokit.users.getAuthenticated();
			return rsp.data.login;
		} catch (e) {
			console.error(e);
			throw e;
		}
	}
}

export class GitHubAuthenticationService {
	constructor(private readonly tauri: Tauri) {}

	async initDeviceOauth() {
		return await this.tauri.invoke<Verification>('init_device_oauth');
	}

	async checkAuthStatus(params: { deviceCode: string }) {
		return await this.tauri.invoke<string>('check_auth_status', params);
	}
}

export const [getGitHubUserServiceStore, createGitHubUserServiceStore] = buildContextStore<
	GitHubUserService | undefined
>('githubUserService');

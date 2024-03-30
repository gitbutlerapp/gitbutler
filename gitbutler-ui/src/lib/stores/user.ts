import { resetPostHog, setPostHogUser } from '$lib/analytics/posthog';
import { resetSentry, setSentryUser } from '$lib/analytics/sentry';
import { User, type CloudClient } from '$lib/backend/cloud';
import { invoke } from '$lib/backend/ipc';
import { sleep } from '$lib/utils/sleep';
import { openExternalUrl } from '$lib/utils/url';
import { plainToInstance } from 'class-transformer';
import { writable, type Readable, type Writable, derived } from 'svelte/store';

export class UserService {
	readonly user: Writable<User | undefined> = writable();
	readonly accessToken: Readable<string | undefined>;
	readonly error: Writable<any> = writable();

	constructor(private cloud: CloudClient) {
		this.loadUser();

		this.accessToken = derived(this.user, (user) => {
			user?.access_token;
		});
	}

	async loadUser() {
		try {
			const userData = await invoke<User | undefined>('get_user');
			if (!userData) return;

			const user = plainToInstance(User, userData);
			this.user.set(user);
			setPostHogUser(user);
			setSentryUser(user);
		} catch (e) {
			this.error.set(e);
		}
	}

	async setUser(user: User | undefined) {
		if (user) await invoke('set_user', { user });
		else await this.clearUser();

		this.user.set(user);
	}

	async clearUser() {
		await invoke('delete_user');
	}

	async logout() {
		await this.clearUser();

		this.user.set(undefined);

		resetPostHog();
		resetSentry();
	}

	async login(): Promise<User | undefined> {
		this.logout();

		const token = await this.cloud.createLoginToken();
		openExternalUrl(token.url);

		// Assumed min time for login flow
		await sleep(4000);

		const user = await this.pollForUser(token.token);
		this.setUser(user);

		return user;
	}

	private async pollForUser(token: string): Promise<User | undefined> {
		let apiUser: User | null;
		for (let i = 0; i < 120; i++) {
			apiUser = await this.cloud.getLoginUser(token).catch(() => null);
			if (apiUser) {
				this.setUser(apiUser);
				return apiUser;
			}
			await sleep(1000);
		}
	}
}

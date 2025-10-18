import { resetSentry, setSentryUser } from '$lib/analytics/sentry';
import { showError } from '$lib/notifications/toasts';
import { sleep } from '$lib/utils/sleep';
import { InjectionToken } from '@gitbutler/core/context';
import { type HttpClient } from '@gitbutler/shared/network/httpClient';
import { copyToClipboard } from '@gitbutler/ui/utils/clipboard';
import { derived, get, readable, writable, type Readable } from 'svelte/store';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { IBackend } from '$lib/backend';
import type { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import type { User } from '$lib/user/user';
import type { ApiUser } from '@gitbutler/shared/users/types';

export type LoginToken = {
	/** Used for polling the user; should NEVER be sent to the browser. */
	token: string;
	browser_token: string;
	expires: string;
	url: string;
};

export const USER_SERVICE = new InjectionToken<UserService>('UserService');

export class UserService {
	readonly loading = writable(false);

	readonly user = writable<User | undefined>(undefined, () => {
		this.refresh();
	});
	readonly userLogin = derived<Readable<User | undefined>, string | undefined>(
		this.user,
		(user, set) => {
			if (user) {
				this.getUser().then((user) => set(user.login ?? undefined));
			} else {
				set(undefined);
			}
		}
	);
	readonly error = writable();

	async refresh() {
		const user = await this.backend.invoke<User | undefined>('get_user');
		if (user) {
			this.tokenMemoryService.setToken(user.access_token);
			this.user.set(user);
			await this.posthog.setPostHogUser({ id: user.id, email: user.email, name: user.name });
			setSentryUser(user);
			return user;
		}

		this.posthog.setAnonymousPostHogUser();
		this.user.set(undefined);
	}

	constructor(
		private backend: IBackend,
		private httpClient: HttpClient,
		private tokenMemoryService: TokenMemoryService,
		private posthog: PostHogWrapper
	) {}

	async setUser(user: User | undefined) {
		if (user) {
			await this.backend.invoke('set_user', { user });
			this.tokenMemoryService.setToken(user.access_token);
		} else {
			await this.clearUser();
		}
		this.user.set(user);
	}

	private async clearUser() {
		await this.backend.invoke('delete_user');
	}

	async logout() {
		await this.clearUser();
		this.user.set(undefined);
		this.tokenMemoryService.setToken(undefined);
		await this.posthog.resetPostHog();
		resetSentry();
	}

	private async loginCommon(
		action: (url: string) => void,
		aborted: Readable<boolean>
	): Promise<User | undefined> {
		this.logout();
		this.loading.set(true);
		try {
			// Create login token
			const token = await this.httpClient.post<LoginToken>('login/token.json');
			const url = new URL(token.url);
			url.host = this.httpClient.apiUrl.host;

			action(url.toString());

			const user = await this.pollForUser(token.token, aborted);
			this.tokenMemoryService.setToken(undefined);
			this.setUser(user);

			return user;
		} catch (err) {
			console.error(err);
			showError('Error occurred while logging in', err);
		} finally {
			this.loading.set(false);
		}
	}

	async login(aborted: Readable<boolean> = readable(false)): Promise<User | undefined> {
		return await this.loginCommon((url) => {
			this.backend.openExternalUrl(url);
		}, aborted);
	}

	async loginAndCopyLink(aborted: Readable<boolean> = readable(false)): Promise<User | undefined> {
		return await this.loginCommon((url) => {
			setTimeout(() => {
				copyToClipboard(url);
			}, 0);
		}, aborted);
	}

	private async pollForUser(token: string, aborted: Readable<boolean>): Promise<User | undefined> {
		const pollingDuration = 20 * 60 * 1000; // 20 minutes
		const pollingFrequency = 5 * 1000; // 5 seconds

		let apiUser: User | null;
		for (let i = 0; i < pollingDuration / pollingFrequency; i++) {
			if (get(aborted)) return;
			apiUser = await this.getLoginUser(token).catch(() => null);
			if (apiUser) {
				this.setUser(apiUser);
				return apiUser;
			}
			await sleep(pollingFrequency);
		}

		throw new Error('Login token expired. Please try loging in again');
	}

	// TODO: Remove token from URL, we don't want that leaking into logs.
	private async getLoginUser(token: string): Promise<User> {
		return await this.httpClient.get(`login/user/${token}.json`);
	}

	async getUser(): Promise<ApiUser> {
		return await this.httpClient.get('user.json');
	}

	async updateUser(params: {
		name?: string;
		picture?: File;
		website?: string;
		twitter?: string;
		bluesky?: string;
		timezone?: string;
		location?: string;
		emailShare?: boolean;
	}): Promise<any> {
		const formData = new FormData();
		if (params.name) formData.append('name', params.name);
		if (params.picture) formData.append('avatar', params.picture);
		if (params.website !== undefined) formData.append('website', params.website);
		if (params.twitter !== undefined) formData.append('twitter', params.twitter);
		if (params.bluesky !== undefined) formData.append('bluesky', params.bluesky);
		if (params.timezone !== undefined) formData.append('timezone', params.timezone);
		if (params.location !== undefined) formData.append('location', params.location);
		if (params.emailShare !== undefined)
			formData.append('email_share', params.emailShare.toString());

		// Content Type must be unset for the right form-data border to be set automatically
		return await this.httpClient.put('user.json', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});
	}
}

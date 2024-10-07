import { resetPostHog, setPostHogUser } from '$lib/analytics/posthog';
import { resetSentry, setSentryUser } from '$lib/analytics/sentry';
import { API_URL, type HttpClient } from '$lib/backend/httpClient';
import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { copyToClipboard } from '$lib/utils/clipboard';
import { sleep } from '$lib/utils/sleep';
import { openExternalUrl } from '$lib/utils/url';
import { plainToInstance } from 'class-transformer';
import { derived, writable } from 'svelte/store';

export type LoginToken = {
	token: string;
	expires: string;
	url: string;
};

export class UserService {
	readonly loading = writable(false);

	readonly user = writable<User | undefined>(undefined, () => {
		this.refresh();
	});
	readonly error = writable();

	async refresh() {
		const userData = await invoke<User | undefined>('get_user');
		if (userData) {
			const user = plainToInstance(User, userData);
			this.user.set(user);
			setPostHogUser(user);
			setSentryUser(user);
			return user;
		}
		this.user.set(undefined);
	}
	readonly accessToken$ = derived(this.user, (user) => {
		user?.github_access_token;
	});

	constructor(private httpClient: HttpClient) {}

	async setUser(user: User | undefined) {
		console.log('Setting user - github access token', user?.github_access_token);
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

	private async loginCommon(action: (url: string) => void): Promise<User | undefined> {
		this.logout();
		this.loading.set(true);
		try {
			// Create login token
			const token = await this.httpClient.post<LoginToken>('login/token.json');
			const url = new URL(token.url);
			url.host = API_URL.host;

			action(url.toString());

			// Assumed min time for login flow
			await sleep(4000);

			const user = await this.pollForUser(token.token);
			this.setUser(user);

			return user;
		} catch (err) {
			console.error(err);
			showError('Something went wrong', err);
		} finally {
			this.loading.set(false);
		}
	}

	async login(): Promise<User | undefined> {
		return await this.loginCommon((url) => {
			openExternalUrl(url);
		});
	}

	async loginAndCopyLink(): Promise<User | undefined> {
		return await this.loginCommon((url) => {
			setTimeout(() => {
				copyToClipboard(url);
			}, 0);
		});
	}

	async pollForUser(token: string): Promise<User | undefined> {
		let apiUser: User | null;
		for (let i = 0; i < 120; i++) {
			apiUser = await this.getLoginUser(token).catch(() => null);
			if (apiUser) {
				this.setUser(apiUser);
				return apiUser;
			}
			await sleep(1000);
		}
	}

	// TODO: Remove token from URL, we don't want that leaking into logs.
	async getLoginUser(token: string): Promise<User> {
		return await this.httpClient.get(`login/user/${token}.json`);
	}

	async getUser(token: string): Promise<User> {
		return await this.httpClient.get('user.json', { token });
	}

	async updateUser(token: string, params: { name?: string; picture?: File }): Promise<any> {
		const formData = new FormData();
		if (params.name) formData.append('name', params.name);
		if (params.picture) formData.append('avatar', params.picture);

		// Content Type must be unset for the right form-data border to be set automatically
		return await this.httpClient.put('user.json', {
			body: formData,
			headers: { 'Content-Type': undefined },
			token
		});
	}
}

export interface GitHubLogin {
	label: string | undefined;
	accessToken: string;
	username: string;
}

export class User {
	id!: number;
	name: string | undefined;
	given_name: string | undefined;
	family_name: string | undefined;
	email!: string;
	picture!: string;
	locale!: string;
	created_at!: string;
	updated_at!: string;
	access_token!: string;
	role: string | undefined;
	supporter!: boolean;
	/**
	 * Selected GitHub access token.
	 */
	github_access_token: string | undefined;
	/**
	 * Selected GitHub username.
	 */
	github_username: string | undefined;
	/**
	 * List of avaiable GitHub logins.
	 *
	 * By default this is empty, but is populated when adding secondary GitHub logins.
	 */
	github_logins!: GitHubLogin[];
}

interface BaseGitHubLoginListItem extends GitHubLogin {
	selected: boolean;
}

interface SelectedGitHubLoginListItem extends BaseGitHubLoginListItem {
	selected: true;
}

interface UnselectedGitHubLoginListItem extends BaseGitHubLoginListItem {
	selected: false;
}

export type GitHubLoginListItem = SelectedGitHubLoginListItem | UnselectedGitHubLoginListItem;

export function getGitHubLoginList(user: User | undefined): GitHubLoginListItem[] {
	const list: GitHubLoginListItem[] = [];
	if (!user) return list;

	for (const login of user.github_logins) {
		if (login.username !== user.github_username) {
			list.push({ ...login, selected: false });
		}
	}

	if (user.github_access_token) {
		list.push({
			label: undefined,
			accessToken: user.github_access_token,
			username: user.github_username ?? 'unknown',
			selected: true
		});
	}

	return list.sort((a, b) => a.username.localeCompare(b.username));
}

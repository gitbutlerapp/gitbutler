import { resetPostHog, setPostHogUser } from '$lib/analytics/posthog';
import { resetSentry, setSentryUser } from '$lib/analytics/sentry';
import { API_URL, type HttpClient } from '$lib/backend/httpClient';
import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { observableToStore } from '$lib/rxjs/store';
import { sleep } from '$lib/utils/sleep';
import { openExternalUrl } from '$lib/utils/url';
import { plainToInstance } from 'class-transformer';
import { Observable, Subject, distinct, map, merge, shareReplay } from 'rxjs';
import { writable } from 'svelte/store';
import type { Readable } from 'svelte/motion';

export type LoginToken = {
	token: string;
	expires: string;
	url: string;
};

export class UserService {
	private reset$ = new Subject<User | undefined>();
	readonly loading = writable(false);

	private user$ = merge(
		new Observable<User | undefined>((subscriber) => {
			invoke<User | undefined>('get_user').then((userData) => {
				if (userData) {
					const user = plainToInstance(User, userData);
					subscriber.next(user);
					setPostHogUser(user);
					setSentryUser(user);
				}
			});
		}),
		this.reset$
	).pipe(shareReplay(1));

	readonly accessToken$ = this.user$.pipe(
		map((user) => user?.github_access_token),
		distinct()
	);

	readonly user: Readable<User | undefined>;
	readonly error: Readable<string | undefined>;

	constructor(private httpClient: HttpClient) {
		[this.user, this.error] = observableToStore(this.user$);
	}

	async setUser(user: User | undefined) {
		if (user) await invoke('set_user', { user });
		else await this.clearUser();
		this.reset$.next(user);
	}

	async clearUser() {
		await invoke('delete_user');
	}

	async logout() {
		await this.clearUser();
		this.reset$.next(undefined);
		resetPostHog();
		resetSentry();
	}

	async login(): Promise<User | undefined> {
		this.logout();
		this.loading.set(true);
		try {
			// Create login token
			const token = await this.httpClient.post<LoginToken>('login/token.json');
			const url = new URL(token.url);
			url.host = API_URL.host;
			openExternalUrl(url.toString());

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
	github_access_token: string | undefined;
	github_username: string | undefined;
}

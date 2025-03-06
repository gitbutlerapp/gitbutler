import { setSentryUser } from '$lib/analytics/sentry';
import { get, writable, type Writable } from 'svelte/store';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

export interface User {
	id: number;
	login: string | undefined;
	name: string;
	email: string;
	created_at: Date;
	picture: string;
	supporter: boolean;
}

export class UserService {
	user: Writable<User | undefined> = writable<User | undefined>(undefined, (set) => {
		this.fetchUser()
			.then((data) => {
				this.error.set(undefined);
				set(data);
			})
			.catch((err) => {
				this.error.set(err);
			});
	});

	readonly error = writable();

	constructor(private readonly httpClient: HttpClient) {
		httpClient.authenticationAvailable.subscribe((available) => {
			if (available && get(this.user) === undefined) {
				// If the authentication availability changes, refetch the use
				this.fetchUser()
					.then((data) => {
						this.error.set(undefined);
						this.user.set(data);
					})
					.catch((err) => {
						this.error.set(err);
					});
			}
		});
	}

	private async fetchUser() {
		const user = await this.httpClient.get<User>('/api/user');
		setSentryUser(user);

		return user;
	}

	clearUser() {
		this.user.set(undefined);
	}

	async updateUser(params: { name?: string; picture?: File }): Promise<any> {
		const formData = new FormData();
		if (params.name) formData.append('name', params.name);
		if (params.picture) formData.append('avatar', params.picture);

		// Content Type must be unset for the right form-data border to be set automatically
		return await this.httpClient.put('user.json', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});
	}
}

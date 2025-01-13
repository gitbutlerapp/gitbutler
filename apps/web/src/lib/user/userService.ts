import { setSentryUser } from '$lib/analytics/sentry';
import { writable, type Writable } from 'svelte/store';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

export interface User {
	id: string;
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

	constructor(private readonly httpClient: HttpClient) {}

	private async fetchUser() {
		const user = await this.httpClient.get<User>('/api/user');
		setSentryUser(user);

		return user;
	}

	clearUser() {
		this.user.set(undefined);
	}
}

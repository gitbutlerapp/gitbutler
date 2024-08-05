import { AuthService } from '$lib/auth/authService';
import { setSentryUser } from '$lib/analytics/sentry';
import { get, writable } from 'svelte/store';
import { env } from '$env/dynamic/public';

export interface User {
	id: string;
	name: string;
	email: string;
	created_at: Date;
	picture: string;
	supporter: boolean;
}

export class UserService {
	private authService = new AuthService();
	private userStore = writable<User | undefined>(undefined, (set) => {
		this.fetchUser()
			.then((user) => {
				this.error.set(undefined);
				set(user);
			})
			.catch((err) => {
				this.error.set(err);
			});
	});

	readonly error = writable();

	constructor() {}

	private async fetchUser() {
		if (this.authService.token) {
			const userResponse = await fetch(env.PUBLIC_APP_HOST + 'api/user', {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': this.authService.token
				}
			});
			if (!userResponse.ok) {
				throw new Error('Failed to fetch user');
			}

			const userBody = await userResponse.json();
			setSentryUser(userBody);

			return userBody;
		}
	}

	get user() {
		return get(this.userStore);
	}

	clearUser() {
		this.userStore.set(undefined);
	}
}

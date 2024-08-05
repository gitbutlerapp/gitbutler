import { AuthService } from '$lib/auth/authService';
import { setSentryUser } from '$lib/analytics/sentry';
import { writable } from 'svelte/store';
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
	user = writable<User | undefined>(undefined, (set) => {
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

	constructor(readonly authService: AuthService) {}

	private async fetchUser() {
		const authToken = this.authService.getToken();
		if (authToken) {
			const userResponse = await fetch(env.PUBLIC_APP_HOST + 'api/user', {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': authToken
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

	clearUser() {
		this.user.set(undefined);
	}
}

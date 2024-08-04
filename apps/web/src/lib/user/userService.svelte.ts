import { createAuthService } from '$lib/auth/authService.svelte';
import { setSentryUser } from '$lib/analytics/sentry';
import { env } from '$env/dynamic/public';

export interface User {
	id: string;
	name: string;
	email: string;
	created_at: Date;
	picture: string;
	supporter: boolean;
}

let user = $state<User | undefined>();

export function createUserService() {
	const authService = createAuthService();

	if (!user?.id) {
		fetchUser();
	}

	function fetchUser() {
		if (authService.token) {
			fetch(env.PUBLIC_APP_HOST + 'api/user', {
				method: 'GET',
				headers: {
					'X-AUTH-TOKEN': authService.token || ''
				}
			})
				.then(async (response) => {
					if (!response.ok) {
						throw new Error('Failed to fetch user');
					}

					return await response.json();
				})
				.then((data) => {
					setUser(data);
					setSentryUser(data);
				});
		}
	}

	function setUser(data: User) {
		user = data;
	}

	function clearUser() {
		user = undefined;
	}

	return {
		get user() {
			return user;
		},
		setUser,
		clearUser
	};
}

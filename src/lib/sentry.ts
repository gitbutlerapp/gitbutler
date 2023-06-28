import { setUser } from '@sentry/sveltekit';
import type { User } from '$lib/api';

export default () => {
	return {
		identify: (user: User | null) => {
			if (user) {
				setUser({
					id: user.id.toString(),
					email: user.email,
					username: user.name
				});
			} else {
				setUser(null);
			}
		}
	};
};

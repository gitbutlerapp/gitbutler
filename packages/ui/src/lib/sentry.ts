import { setUser } from '@sentry/sveltekit';
import type { User } from './api/cloud';

export default () => {
	return {
		identify: (user: User | undefined) => {
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

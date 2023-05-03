import { setUser } from '@sentry/sveltekit';
import type { User } from '$lib/api';
import * as log from '$lib/log';

export default () => {
	return {
		identify: (user: User | null) => {
			if (user) {
				log.info(`sentry identify`);
				setUser({
					id: user.id.toString(),
					email: user.email,
					username: user.name
				});
			} else {
				log.info(`sentry reset`);
				setUser(null);
			}
		}
	};
};

import posthog from 'posthog-js';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';
import type { User } from '$lib/api';
import * as log from '$lib/log';

export default () => {
	const instance = posthog.init(PUBLIC_POSTHOG_API_KEY, {
		api_host: 'https://eu.posthog.com',
		capture_performance: false
	});
	log.info('posthog initialized');
	return {
		identify: (user: User | null) => {
			if (user) {
				log.info('posthog identify', {
					id: user.id,
					name: user.name,
					email: user.email
				});
				instance?.identify(`user_${user.id}`, {
					email: user.email,
					name: user.name
				});
			} else {
				log.info('posthog reset');
				instance?.capture('log-out');
				instance?.reset();
			}
		}
	};
};

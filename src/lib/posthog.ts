import posthog from 'posthog-js';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';
import type { User } from '$lib/api';
import { getVersion, getName } from '@tauri-apps/api/app';

export default () => {
	const instance = posthog.init(PUBLIC_POSTHOG_API_KEY, {
		api_host: 'https://eu.posthog.com',
		capture_performance: false,
		request_batching: true,
		persistence: 'localStorage',
		on_xhr_error: () => {
			// noop
		},
		loaded: (instance) =>
			Promise.all([getVersion(), getName()]).then(([version, name]) => {
				instance.register_for_session({
					appName: name,
					appVersion: version
				});
			})
	});
	return {
		identify: (user: User | null) => {
			if (user) {
				instance?.identify(`user_${user.id}`, {
					email: user.email,
					name: user.name
				});
			} else {
				instance?.capture('log-out');
				instance?.reset();
			}
		}
	};
};

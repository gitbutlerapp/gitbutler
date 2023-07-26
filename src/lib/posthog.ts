import posthog from 'posthog-js';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';
import type { User } from '$lib/api';
import { getVersion, getName } from '@tauri-apps/api/app';

export default async function () {
	const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
	return new Promise((resolve, _reject) => {
		posthog.init(PUBLIC_POSTHOG_API_KEY, {
			api_host: 'https://eu.posthog.com',
			capture_performance: false,
			request_batching: true,
			persistence: 'localStorage',
			on_xhr_error: () => {
				// noop
			},
			loaded: (instance) => {
				console.log('Posthog loaded');
				instance.register_for_session({
					appName,
					appVersion
				});
				resolve({
					identify: (user: User | null) => {
						if (user) {
							instance.identify(`user_${user.id}`, {
								email: user.email,
								name: user.name
							});
						} else {
							instance.capture('log-out');
							instance.reset();
						}
					}
				});
			}
		});
	});
}

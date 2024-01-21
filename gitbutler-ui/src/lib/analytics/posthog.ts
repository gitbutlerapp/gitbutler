import { posthog } from 'posthog-js';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';
import type { User } from '../backend/cloud';
import { getVersion, getName } from '@tauri-apps/api/app';

export async function initPostHog() {
	const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
	new Promise((resolve, _reject) => {
		posthog.init(PUBLIC_POSTHOG_API_KEY, {
			api_host: 'https://eu.posthog.com',
			disable_session_recording: appName !== 'GitButler', // only record sessions in production
			capture_performance: false,
			request_batching: true,
			persistence: 'localStorage',
			on_xhr_error: () => {
				// noop
			},
			loaded: (instance) => {
				instance.register_for_session({
					appName,
					appVersion
				});
				resolve({
					identify: (user: User | undefined) => {
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

export function setPostHogUser(user: User) {
	posthog.identify(`user_${user.id}`, {
		email: user.email,
		name: user.name
	});
}

export function resetPostHog() {
	posthog.capture('logout');
	posthog.reset();
}

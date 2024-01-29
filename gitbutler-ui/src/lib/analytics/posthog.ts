import { getVersion, getName } from '@tauri-apps/api/app';
import { posthog } from 'posthog-js';
import type { User } from '../backend/cloud';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';

export async function initPostHog() {
	const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
	posthog.init(PUBLIC_POSTHOG_API_KEY, {
		api_host: 'https://eu.posthog.com',
		disable_session_recording: appName !== 'GitButler', // only record sessions in production
		capture_performance: false,
		request_batching: true,
		persistence: 'localStorage',
		on_xhr_error: (e) => {
			console.log('posthog error', e);
		}
	});
	posthog.register({
		appName,
		appVersion
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

export function capture(eventName: string, properties: any = undefined) {
	console.log('PostHog event', eventName, properties);
	posthog.capture(eventName, properties);
}

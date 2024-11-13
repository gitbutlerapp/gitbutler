import { getVersion, getName } from '@tauri-apps/api/app';
import { posthog } from 'posthog-js';
import type { User } from '$lib/stores/user';
import type { RepoInfo } from '$lib/url/gitUrl';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';

export async function initPostHog() {
	const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
	posthog.init(PUBLIC_POSTHOG_API_KEY, {
		api_host: 'https://eu.posthog.com',
		disable_session_recording: true,
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
	posthog.capture(eventName, properties);
}

/**
 * Include repo information for all events for the remainder of the session,
 * or until cleared.
 */
export function setPostHogRepo(repo: RepoInfo | undefined) {
	if (repo) {
		posthog.register_for_session({ repoDomain: repo.domain, repoHash: repo.hash });
	} else {
		posthog.unregister_for_session('repoDomain');
		posthog.unregister_for_session('repoHash');
	}
}

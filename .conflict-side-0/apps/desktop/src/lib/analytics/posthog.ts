import { PostHog, posthog } from 'posthog-js';
import type { RepoInfo } from '$lib/url/gitUrl';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';

export class PostHogWrapper {
	private _instance: PostHog | void = undefined;

	constructor() {}

	capture(...args: Parameters<typeof posthog.capture>) {
		this._instance?.capture(...args);
	}

	async init(appName: string, appVersion: string) {
		this._instance = posthog.init(PUBLIC_POSTHOG_API_KEY, {
			api_host: 'https://eu.posthog.com',
			autocapture: false,
			disable_session_recording: true,
			capture_performance: false,
			request_batching: true,
			persistence: 'localStorage',
			on_xhr_error: (e) => {
				console.error('posthog error', e);
			}
		});
		posthog.register({
			appName,
			appVersion
		});
	}

	setPostHogUser(params: { id: number; email?: string; name?: string }) {
		const { id, email, name } = params;
		this._instance?.identify(`user_${id}`, {
			email,
			name
		});
	}

	resetPostHog() {
		this._instance?.capture('logout');
		this._instance?.reset();
	}

	/**
	 * Include repo information for all events for the remainder of the session,
	 * or until cleared.
	 */
	setPostHogRepo(repo: RepoInfo | undefined) {
		if (repo) {
			this._instance?.register_for_session({ repoDomain: repo.domain, repoHash: repo.hash });
		} else {
			this._instance?.unregister_for_session('repoDomain');
			this._instance?.unregister_for_session('repoHash');
		}
	}
}

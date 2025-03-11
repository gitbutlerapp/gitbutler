import * as Sentry from '@sentry/sveltekit';
import type { User } from '$lib/user/userService';
import { dev } from '$app/environment';

export function initSentry() {
	// eslint-disable-next-line
	if (!dev && false) {
		// TODO: Currently explicitly disabled until we want to collect Sentry errors
		Sentry.init({
			dsn: 'https://2274a916bfc140b8bc86b6f7f27d1c20@o4504644069687296.ingest.us.sentry.io/4504644070998016',
			tracesSampleRate: 1.0
		});
	}
}

export function setSentryUser(user: User) {
	if (Sentry.getClient()) {
		Sentry.setUser({
			id: user.id,
			email: user.email,
			username: user.name
		});
	}
}

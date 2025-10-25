import { dev } from '$app/environment';
import * as Sentry from '@sentry/sveltekit';
import type { User } from '$lib/user/user';
import { PUBLIC_SENTRY_ENVIRONMENT } from '$env/static/public';

const { setUser, init } = Sentry;

export function initSentry() {
	init({
		enabled: !dev,
		dsn: 'https://a35bbd6688a3a8f76e4956c6871f414a@o4504644069687296.ingest.sentry.io/4505976067129344',
		environment: PUBLIC_SENTRY_ENVIRONMENT,
		tracesSampleRate: 0,
		tracePropagationTargets: ['localhost', /gitbutler\.com/i]
	});
}

export function setSentryUser(user: User) {
	setUser({
		id: user.id.toString(),
		email: user.email,
		username: user.name
	});
}

export function resetSentry() {
	setUser(null);
}

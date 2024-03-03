import { startSpan, setUser, type Span, init } from '@sentry/sveltekit';
import type { User } from '../backend/cloud';
import { dev } from '$app/environment';
import { PUBLIC_SENTRY_ENVIRONMENT } from '$env/static/public';

export function initSentry() {
	init({
		enabled: !dev,
		dsn: 'https://a35bbd6688a3a8f76e4956c6871f414a@o4504644069687296.ingest.sentry.io/4505976067129344',
		environment: PUBLIC_SENTRY_ENVIRONMENT,
		tracesSampleRate: 0.1
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

export function instrument<T>(name: string, callback: (span: Span | undefined) => T): T {
	return startSpan({ name }, callback);
}

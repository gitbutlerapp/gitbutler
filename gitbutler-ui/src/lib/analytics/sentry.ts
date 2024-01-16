import { startSpan, setUser, type Span } from '@sentry/sveltekit';
import type { User } from '../backend/cloud';

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

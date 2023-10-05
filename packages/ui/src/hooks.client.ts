import { handleErrorWithSentry, init } from '@sentry/sveltekit';
import type { NavigationEvent } from '@sveltejs/kit';
import { dev } from '$app/environment';
import { PUBLIC_SENTRY_ENVIRONMENT } from '$env/static/public';

init({
	enabled: !dev,
	dsn: 'https://a35bbd6688a3a8f76e4956c6871f414a@o4504644069687296.ingest.sentry.io/4505976067129344',
	environment: PUBLIC_SENTRY_ENVIRONMENT,
	tracesSampleRate: 1.0
});

function myErrorHandler({ error, event }: { error: any; event: NavigationEvent }) {
	console.error('An error occurred on the client side:', error, event);
}

export const handleError = handleErrorWithSentry(myErrorHandler);

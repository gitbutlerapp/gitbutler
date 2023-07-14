import { handleErrorWithSentry, init } from '@sentry/sveltekit';
import type { NavigationEvent } from '@sveltejs/kit';
import { dev } from '$app/environment';
import { PUBLIC_SENTRY_ENVIRONMENT } from '$env/static/public';

init({
	enabled: !dev,
	dsn: 'https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032',
	environment: PUBLIC_SENTRY_ENVIRONMENT,
	tracesSampleRate: 1.0
});

console.log(`sentry init`);

function myErrorHandler({ error, event }: { error: any; event: NavigationEvent }) {
	console.error('An error occurred on the client side:', error, event);
}

export function handleError() {
	return handleErrorWithSentry(myErrorHandler);
}

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

/**
 * This is not an ideal way of handling unhandled errors, but it's what we have at the moment. The
 * main reason for adding this is that it gives us better stack traces when promises throw errors
 * in reactive statements.
 *
 * See: https://github.com/sveltejs/rfcs/pull/46
 */
const originalUnhandledHandler = window.onunhandledrejection;
window.onunhandledrejection = (event: PromiseRejectionEvent) => {
	console.log('Unhandled exception', event.reason);
	originalUnhandledHandler?.bind(window)(event);
};

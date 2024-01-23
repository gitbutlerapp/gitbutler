import { handleErrorWithSentry } from '@sentry/sveltekit';
import type { NavigationEvent } from '@sveltejs/kit';

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

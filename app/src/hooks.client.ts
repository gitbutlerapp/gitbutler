import { handleErrorWithSentry } from '@sentry/sveltekit';
import { error as logErrorToFile } from 'tauri-plugin-log-api';
import type { NavigationEvent } from '@sveltejs/kit';

function myErrorHandler({ error, event }: { error: any; event: NavigationEvent }) {
    if (typeof window.__TAURI_IPC__ === 'function') {
        console.error(error.message + '\n' + error.stack);
        logErrorToFile(error.toString());
    }
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
    if (typeof window.__TAURI_IPC__ === 'function') {
        logErrorToFile('Unhandled exception: ' + event?.reason + ' ' + event?.reason?.sourceURL);
        console.log('Unhandled exception', event.reason);
        originalUnhandledHandler?.bind(window)(event);
    }
};

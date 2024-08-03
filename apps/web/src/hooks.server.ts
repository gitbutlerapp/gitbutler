import { handleErrorWithSentry, sentryHandle } from '@sentry/sveltekit';
import * as Sentry from '@sentry/sveltekit';
import { sequence } from '@sveltejs/kit/hooks';

Sentry.init({
	dsn: 'https://2274a916bfc140b8bc86b6f7f27d1c20@o4504644069687296.ingest.us.sentry.io/4504644070998016',
	tracesSampleRate: 1.0
});

export const handle = sequence(sentryHandle());

export const handleError = handleErrorWithSentry();

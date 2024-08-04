import { initSentry } from '$lib/analytics/sentry';
import { handleErrorWithSentry, sentryHandle } from '@sentry/sveltekit';
import { sequence } from '@sveltejs/kit/hooks';

initSentry();

export const handle = sequence(sentryHandle());

export const handleError = handleErrorWithSentry();

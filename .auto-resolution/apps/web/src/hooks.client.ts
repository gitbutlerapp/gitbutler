import { initSentry } from '$lib/analytics/sentry';
import { handleErrorWithSentry } from '@sentry/sveltekit';

initSentry();

export const handleError = handleErrorWithSentry();

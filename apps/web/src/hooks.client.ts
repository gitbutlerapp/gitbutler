import { initPostHog } from "$lib/analytics/posthog";
import { initSentry } from "$lib/analytics/sentry";
import { handleErrorWithSentry } from "@sentry/sveltekit";

initSentry();
initPostHog();

export const handleError = handleErrorWithSentry();

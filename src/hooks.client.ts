import posthog from "posthog-js";
import { PUBLIC_POSTHOG_API_KEY } from "$env/static/public";

async function setupPostHog() {
    console.log("Initializing PostHog");
    posthog.init(PUBLIC_POSTHOG_API_KEY, {
        api_host: "https://eu.posthog.com",
        capture_performance: false,
    });
}

// Initialize the database *once*
const phSetup = setupPostHog();

export async function handle({ request, render }) {
    // Wait for the posthog to initialize the first time
    // or instantly get the result of when it was initialized
    const ph = await phSetup;
    return await render(request);
}

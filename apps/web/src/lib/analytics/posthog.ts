import posthog from "posthog-js";
import type { User } from "$lib/user/userService";

const API_KEY = "phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG";

export function initPostHog() {
	if (location.hostname !== "gitbutler.com") return;
	posthog.init(API_KEY, {
		api_host: "https://eu.posthog.com",
		defaults: "2026-06-25",
		cookie_persisted_properties: ["app_distinct_id"],
	});
	recordAppDistinctId();
}

function recordAppDistinctId() {
	const url = new URL(location.href);
	const appDistinctId = url.searchParams.get("did");
	if (!appDistinctId) return;
	posthog.register({ app_distinct_id: appDistinctId });
	url.searchParams.delete("did");
	history.replaceState(history.state, "", url);
}

export function setPostHogUser(user: User) {
	if (!posthog.__loaded) return;
	posthog.identify(`user_${user.id}`, { email: user.email, name: user.name });
}

export function resetPostHogUser() {
	if (!posthog.__loaded) return;
	posthog.reset();
}

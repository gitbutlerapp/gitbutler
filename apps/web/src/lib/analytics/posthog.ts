import { POSTHOG_API_KEY } from "$lib/analytics/posthogKey";
import posthog from "posthog-js";
import type { User } from "$lib/user/userService";

export function initPostHog() {
	if (location.hostname !== "gitbutler.com") return;
	posthog.init(POSTHOG_API_KEY, {
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

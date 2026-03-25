import { createFileRoute, notFound } from "@tanstack/react-router";

import { handleWatcher } from "#ui/watcher.ts";

export const Route = createFileRoute("/project/$id")({
	beforeLoad: ({ matches, routeId }) => {
		// We don't want an index route.
		if (matches.at(-1)?.routeId === routeId) throw notFound();
	},
	loader: async ({ params, context }) => {
		const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
			handleWatcher(event, params.id, context.queryClient),
		);
		return { subscriptionId };
	},
	onLeave: ({ loaderData }) => {
		if (loaderData) void window.lite.watcherUnsubscribe(loaderData.subscriptionId);
	},
});

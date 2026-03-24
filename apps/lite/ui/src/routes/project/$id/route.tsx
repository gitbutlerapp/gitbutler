import { createFileRoute } from "@tanstack/react-router";

import { handleWatcher } from "#ui/watcher.ts";

export const Route = createFileRoute("/project/$id")({
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

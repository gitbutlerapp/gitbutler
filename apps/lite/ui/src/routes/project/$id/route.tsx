import { createRoute } from "@tanstack/react-router";

import { rootRoute } from "#ui/routes/__root.tsx";
import { handleWatcher } from "#ui/watcher.ts";

export const projectRoute = createRoute({
	getParentRoute: () => rootRoute,
	loader: async ({ params, context }) => {
		const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
			handleWatcher(event, params.id, context.queryClient),
		);
		return { subscriptionId };
	},
	onLeave: ({ loaderData }) => {
		if (loaderData) void window.lite.watcherUnsubscribe(loaderData.subscriptionId);
	},
	path: "/project/$id",
});

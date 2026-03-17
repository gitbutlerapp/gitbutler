import { createRoute } from "@tanstack/react-router";

import { rootRoute } from "#ui/routes/root.tsx";
import { removeWatcherSubscription, subscribeToProject } from "#ui/watcher.ts";

export const projectRootRoute = createRoute({
	getParentRoute: () => rootRoute,
	loader: async ({ params, context }) => {
		// When loading the project root, subscribe to its events.
		const projectId = params.id;
		const subscriptionId = await subscribeToProject(projectId, context.queryClient);
		return { subscriptionId, projectId };
	},
	// oxlint-disable-next-line typescript/no-misused-promises
	onLeave: async ({ loaderData }) => {
		// When leaving the project root, unsubscribe.
		if (loaderData) await removeWatcherSubscription(loaderData.subscriptionId);
	},
	path: "/project/$id",
});

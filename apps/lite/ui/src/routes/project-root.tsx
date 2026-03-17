import { createRoute } from "@tanstack/react-router";

import { rootRoute } from "#ui/routes/root.tsx";
import { removeWatcherSubscription, subscribeToProject } from "#ui/watcher.ts";

export const projectRootRoute = createRoute({
	getParentRoute: () => rootRoute,
	loader: async ({ params, context }) => {
		const subscriptionId = await subscribeToProject(params.id, context.queryClient);
		return { subscriptionId };
	},
	// oxlint-disable-next-line typescript/no-misused-promises
	onLeave: async ({ loaderData }) => {
		if (loaderData) await removeWatcherSubscription(loaderData.subscriptionId);
	},
	path: "/project/$id",
});

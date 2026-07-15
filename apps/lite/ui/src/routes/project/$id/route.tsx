import { createRoute, notFound, Outlet } from "@tanstack/react-router";
import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { handleWatcher } from "#ui/watcher.ts";
import { store } from "#ui/store.ts";

export const Route = createRoute({
	getParentRoute: () => rootRoute,
	path: "project/$id",
	remountDeps: ({ params }) => params.id,
	// Needed for `remountDeps` to work.
	component: () => <Outlet />,
	beforeLoad: ({ matches, routeId }) => {
		// We don't want an index route.
		if (matches.at(-1)?.routeId === routeId) throw notFound();
	},
	loader: async ({ params }) => {
		// Allow the route to render and handle failure via its queries.
		try {
			const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
				handleWatcher(event, params.id, store.dispatch),
			);
			return { subscriptionId };
		} catch {
			return { subscriptionId: undefined };
		}
	},
	onLeave: ({ loaderData }) => {
		if (loaderData?.subscriptionId !== undefined)
			void window.lite.watcherUnsubscribe(loaderData.subscriptionId);
	},
});

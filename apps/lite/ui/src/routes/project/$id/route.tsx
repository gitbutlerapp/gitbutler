import { createRoute, notFound, Outlet, redirect } from "@tanstack/react-router";
import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { handleProjectEvent } from "#ui/project-events.ts";

export const Route = createRoute({
	getParentRoute: () => rootRoute,
	path: "project/$id",
	remountDeps: ({ params }) => params.id,
	// Needed for `remountDeps` to work.
	component: () => <Outlet />,
	beforeLoad: async ({ matches, routeId, params }) => {
		// We don't want an index route.
		if (matches.at(-1)?.routeId === routeId) throw notFound();

		// The id decodes to a path, and URLs arrive from outside the app, so open
		// only projects it already knows about.
		const projects = await window.lite.listProjectsStateless();
		if (!projects.some((project) => project.id === params.id)) throw redirect({ to: "/" });
	},
	loader: async ({ params, context }) => {
		// Allow the route to render and handle failure via its queries.
		try {
			const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
				handleProjectEvent(event, params.id, context.queryClient),
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

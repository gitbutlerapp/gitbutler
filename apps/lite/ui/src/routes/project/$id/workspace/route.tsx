import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import { Outlet, createRoute } from "@tanstack/react-router";

export const Route = createRoute({
	getParentRoute: () => projectRoute,
	path: "workspace",
	component: Outlet,
});

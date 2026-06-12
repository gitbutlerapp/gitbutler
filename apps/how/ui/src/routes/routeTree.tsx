import { HowHome } from "./HowHome.tsx";
import { createRootRoute, createRoute, Outlet } from "@tanstack/react-router";

const rootRoute = createRootRoute({
	component: Outlet,
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: HowHome,
});

export const routeTree = rootRoute.addChildren([indexRoute]);

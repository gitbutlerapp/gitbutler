import { HowHome } from "./HowHome.tsx";
import { HowSettings } from "./HowSettings.tsx";
import { createRootRoute, createRoute, Outlet } from "@tanstack/react-router";

const rootRoute = createRootRoute({
	component: Outlet,
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: HowHome,
});

const settingsRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/settings",
	component: HowSettings,
});

export const routeTree = rootRoute.addChildren([indexRoute, settingsRoute]);

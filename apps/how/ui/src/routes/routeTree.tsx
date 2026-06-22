import { HowHome, HowProjectPage } from "./HowHome.tsx";
import { HowSettings } from "./HowSettings.tsx";
import { createRootRoute, createRoute, Outlet } from "@tanstack/react-router";

const rootRoute = createRootRoute({
	component: () => (
		<>
			<div className="pt-4">
				<div className="app-drag-region" />
				<Outlet />
			</div>
		</>
	),
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: HowHome,
});

const projectRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/project",
	component: HowProjectPage,
});

const settingsRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/settings",
	component: HowSettings,
});

export const routeTree = rootRoute.addChildren([indexRoute, projectRoute, settingsRoute]);

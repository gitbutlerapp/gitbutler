import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";
import { RootLayout } from "./RootLayout.tsx";

interface RouteContext {
	queryClient: QueryClient;
}

export const Route = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

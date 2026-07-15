import { createRootRoute } from "@tanstack/react-router";
import { RootLayout } from "./RootLayout.tsx";

export const Route = createRootRoute({
	component: RootLayout,
});

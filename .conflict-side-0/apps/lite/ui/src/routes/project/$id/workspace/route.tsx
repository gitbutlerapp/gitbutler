import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import { createRoute } from "@tanstack/react-router";
import { Route as WorkspacePageRoute } from "./WorkspacePage.tsx";

export const Route = createRoute({
	getParentRoute: () => projectRoute,
	path: "workspace",
	component: WorkspacePageRoute,
});

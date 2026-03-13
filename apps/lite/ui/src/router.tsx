import { createRouter } from "@tanstack/react-router";

import { indexRoute } from "#ui/routes/index.tsx";
import { projectBranchesRoute } from "#ui/routes/project-branches.tsx";
import { projectIndexRoute } from "#ui/routes/project-index.tsx";
import { projectRootRoute } from "#ui/routes/project-root.tsx";
import { rootRoute } from "#ui/routes/root.tsx";

const projectRouteTree = projectRootRoute.addChildren([projectIndexRoute, projectBranchesRoute]);
const routeTree = rootRoute.addChildren([indexRoute, projectRouteTree]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}

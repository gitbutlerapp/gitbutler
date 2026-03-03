import { createRouter } from "@tanstack/react-router";

import { indexRoute } from "#ui/routes/index";
import { projectBranchesRoute } from "#ui/routes/project-branches";
import { projectIndexRoute } from "#ui/routes/project-index";
import { projectRootRoute } from "#ui/routes/project-root";
import { rootRoute } from "#ui/routes/root";

const projectRouteTree = projectRootRoute.addChildren([projectIndexRoute, projectBranchesRoute]);
const routeTree = rootRoute.addChildren([indexRoute, projectRouteTree]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}

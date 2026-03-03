import { createRoute } from "@tanstack/react-router";

import { rootRoute } from "#ui/routes/root";

export const projectRootRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/project/$id",
});

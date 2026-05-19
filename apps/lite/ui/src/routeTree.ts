import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { Route as indexRoute } from "#ui/routes/index.tsx";
import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import { Route as projectWorkspaceIndexRoute } from "#ui/routes/project/$id/workspace/index.tsx";
import { Route as projectWorkspaceIntegrateRoute } from "#ui/routes/project/$id/workspace/integrate.tsx";
import { Route as projectWorkspaceRoute } from "#ui/routes/project/$id/workspace/route.tsx";

const projectWorkspaceRouteTree = projectWorkspaceRoute.addChildren([
	projectWorkspaceIndexRoute,
	projectWorkspaceIntegrateRoute,
]);
const projectRouteTree = projectRoute.addChildren([projectWorkspaceRouteTree]);

export const routeTree = rootRoute.addChildren([indexRoute, projectRouteTree]);

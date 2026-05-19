import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { Route as workspaceRoute } from "#ui/routes/project/$id/workspace/route.tsx";
import { WorkspacePage } from "#ui/routes/project/$id/workspace/WorkspacePage.tsx";
import { useSuspenseQuery } from "@tanstack/react-query";
import { createRoute, useParams } from "@tanstack/react-router";
import { FC } from "react";

const WorkspaceIndexPage: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace/" });
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((candidate) => candidate.id === projectId);

	if (!project) return <p>Project not found.</p>;
	return <WorkspacePage />;
};

export const Route = createRoute({
	getParentRoute: () => workspaceRoute,
	path: "/",
	component: WorkspaceIndexPage,
});

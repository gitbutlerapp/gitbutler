import { createRoute, redirect } from "@tanstack/react-router";
import { FC } from "react";
import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";

// oxlint-disable-next-line react/only-export-components
const IndexPage: FC = () => <p>Select a project.</p>;

export const Route = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	loader: async () => {
		const projects = await window.lite.listProjectsStateless();
		const persistedId = window.localStorage.getItem(lastOpenedProjectKey);
		const projectId = projects.some((project) => project.id === persistedId)
			? persistedId
			: projects[0]?.id;

		if (projectId != null)
			throw redirect({ to: "/project/$id/workspace", params: { id: projectId } });

		return null;
	},
	component: IndexPage,
});

import { createRoute, redirect } from "@tanstack/react-router";
import { FC } from "react";

import { lastOpenedProjectKey, rootRoute } from "#ui/routes/root";

const IndexPage: FC = () => <p>Select a project.</p>;

export const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	loader: async () => {
		const projects = await window.lite.listProjects();
		const persistedId = window.localStorage.getItem(lastOpenedProjectKey);
		const projectId = projects.some((project) => project.id === persistedId)
			? persistedId
			: projects[0]?.id;

		if (projectId != null) throw redirect({ to: "/project/$id", params: { id: projectId } });

		return null;
	},
	component: IndexPage,
});

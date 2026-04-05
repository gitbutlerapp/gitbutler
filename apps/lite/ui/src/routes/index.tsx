import { createFileRoute, redirect } from "@tanstack/react-router";
import { FC } from "react";

import { lastOpenedProjectKey } from "#ui/routes/__root.tsx";

const IndexPage: FC = () => <p>Select a project.</p>;

export const Route = createFileRoute("/")({
	loader: async () => {
		const projects = await window.lite.listProjects();
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

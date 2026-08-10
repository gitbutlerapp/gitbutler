import { createRoute, redirect } from "@tanstack/react-router";
import type { FC } from "react";
import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { readLastOpenedProject, readLastPlace } from "#ui/project.ts";

const parseLastSearch = (search: string): Record<string, string> =>
	Object.fromEntries(new URLSearchParams(search));

// oxlint-disable-next-line react/only-export-components -- False positive?
const IndexPage: FC = () => <p>Select a project.</p>;

export const Route = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	loader: async () => {
		const projects = await window.lite.listProjectsStateless();
		const persistedId = readLastOpenedProject();
		const projectId = projects.some((project) => project.id === persistedId)
			? persistedId
			: projects[0]?.id;

		if (projectId != null) {
			// Only the place recorded for this project; anything unparseable in it
			// is dropped by the route's own validation.
			const place = readLastPlace();
			throw redirect({
				to: "/project/$id/workspace",
				params: { id: projectId },
				search: place?.projectId === projectId ? parseLastSearch(place.search) : {},
			});
		}

		return null;
	},
	component: IndexPage,
});

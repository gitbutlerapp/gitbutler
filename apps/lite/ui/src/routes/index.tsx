import { createRoute, redirect } from "@tanstack/react-router";
import { LiteTestId } from "@gitbutler/ui/utils/testIds";
import type { FC } from "react";
import { AddProjectButton } from "#ui/components/AddProjectButton.tsx";
import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { readLastOpenedProject, readLastPlace } from "#ui/project.ts";
import styles from "./IndexPage.module.css";

const parseLastSearch = (search: string): Record<string, string> =>
	Object.fromEntries(new URLSearchParams(search));

// oxlint-disable-next-line react/only-export-components -- False positive?
const IndexPage: FC = () => (
	<section className={styles.page} data-testid={LiteTestId.OnboardingPage}>
		<h1>Welcome to GitButler Lite</h1>
		<p>Add a local Git repository to get started.</p>
		<AddProjectButton testId={LiteTestId.OnboardingAddLocalProjectButton} />
	</section>
);

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

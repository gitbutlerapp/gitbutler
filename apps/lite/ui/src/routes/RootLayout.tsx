import { listProjectsQueryOptions } from "#ui/api/queries.ts";
import { lastOpenedProjectKey } from "#ui/projects/last-opened.ts";
import { ShortcutsBarElementContext, TopBarActionsElementContext } from "#ui/portals.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import { HotkeysProvider } from "@tanstack/react-hotkeys";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import styles from "./RootLayout.module.css";

const ProjectSelect: FC = () => {
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const navigate = useNavigate();
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});
	const selectedProjectId = projectMatch?.params.id;

	return (
		<select
			name="projectId"
			disabled={projects.length === 0}
			value={selectedProjectId ?? ""}
			onChange={(event) => {
				const nextProjectId = event.currentTarget.value;
				void navigate({
					to: "/project/$id/workspace",
					params: { id: nextProjectId },
				});
				window.localStorage.setItem(lastOpenedProjectKey, nextProjectId);
			}}
			className={uiStyles.button}
		>
			<option value="" disabled>
				Select a project
			</option>
			{projects.map((project) => (
				<option key={project.id} value={project.id}>
					{project.title}
				</option>
			))}
		</select>
	);
};

const TopBar: FC<{
	setTopBarActionsElement: (element: HTMLDivElement | null) => void;
}> = ({ setTopBarActionsElement }) => (
	<header className={styles.topBar}>
		<ProjectSelect />
		<div ref={setTopBarActionsElement} className={styles.topBarActions} />
	</header>
);

export const RootLayout: FC = () => {
	const [topBarActionsElement, setTopBarActionsElement] = useState<HTMLDivElement | null>(null);
	const [shortcutsBarElement, setShortcutsBarElement] = useState<HTMLElement | null>(null);

	return (
		<HotkeysProvider>
			<TopBarActionsElementContext.Provider value={topBarActionsElement}>
				<ShortcutsBarElementContext.Provider value={shortcutsBarElement}>
					<main className={styles.layout}>
						<TopBar setTopBarActionsElement={setTopBarActionsElement} />
						<section className={styles.content}>
							<Outlet />
						</section>
						<footer ref={setShortcutsBarElement} className={styles.shortcutsBarFooter} />
					</main>
				</ShortcutsBarElementContext.Provider>
			</TopBarActionsElementContext.Provider>
		</HotkeysProvider>
	);
};

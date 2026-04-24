import { useSuspenseQuery } from "@tanstack/react-query";
import { Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/ShortcutsBar.tsx";
import { isPanelVisible } from "#ui/routes/project/$id/state/layout.ts";
import {
	projectActions,
	selectProjectLayoutState,
} from "#ui/routes/project/$id/state/projectSlice.ts";
import { useAppDispatch, useAppSelector } from "#ui/state/hooks.ts";
import { toggleShowBinding } from "#ui/routes/project/$id/workspace/WorkspaceShortcuts.ts";
import uiStyles from "#ui/ui.module.css";
import styles from "./__root.module.css";
import { listProjectsQueryOptions } from "#ui/api/queries.ts";

export const lastOpenedProjectKey = "lastProject";

interface RouteContext {
	queryClient: QueryClient;
}

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

const TopBarActions: FC = () => {
	const dispatch = useAppDispatch();
	const projectId = useMatch({
		from: "/project/$id",
	}).params.id;
	const layoutState = useAppSelector((state) => selectProjectLayoutState(state, projectId));

	return (
		<div className={styles.topBarActions}>
			<ShortcutButton
				binding={toggleShowBinding}
				type="button"
				className={uiStyles.button}
				aria-pressed={isPanelVisible(layoutState, "show")}
				onClick={() => dispatch(projectActions.togglePanel({ projectId, panel: "show" }))}
			>
				{toggleShowBinding.description}
			</ShortcutButton>
		</div>
	);
};

const TopBar: FC = () => {
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});

	return (
		<header className={styles.topBar}>
			<ProjectSelect />
			{projectMatch && <TopBarActions />}
		</header>
	);
};

const RootLayout: FC = () => {
	const [shortcutsBarPortalNode, setShortcutsBarPortalNode] = useState<HTMLElement | null>(null);

	return (
		<ShortcutsBarPortalContext value={shortcutsBarPortalNode}>
			<main className={styles.layout}>
				<TopBar />
				<section className={styles.content}>
					<Outlet />
				</section>
				<footer className={styles.shortcutsBarFooter} ref={setShortcutsBarPortalNode} />
			</main>
		</ShortcutsBarPortalContext>
	);
};

export const Route = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, use, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";
import { assert } from "#ui/routes/project/$id/-shared.tsx";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import { isPreviewPanelVisible } from "#ui/routes/project/$id/-state/layout.ts";
import {
	ProjectStateContext,
	ProjectStateProvider,
} from "#ui/routes/project/$id/-ProjectState.tsx";
import {
	toggleFullscreenPreviewBinding,
	togglePreviewBinding,
} from "#ui/routes/project/$id/workspace/-WorkspaceShortcuts.ts";
import uiStyles from "#ui/ui.module.css";
import styles from "./__root.module.css";

export const lastOpenedProjectKey = "lastProject";

interface RouteContext {
	queryClient: QueryClient;
}

const ProjectSelect: FC = () => {
	const { data: projects } = useSuspenseQuery({
		queryKey: ["projects"],
		queryFn: () => window.lite.listProjects(),
	});
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

const SidebarNav: FC = () => {
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});
	const selectedProjectId = projectMatch?.params.id;

	if (selectedProjectId === undefined) return null;

	return (
		<nav className={styles.sidebarNav}>
			<Link
				to={"/project/$id/workspace"}
				params={{ id: selectedProjectId }}
				className={styles.navLink}
				activeProps={{ className: styles.navLinkActive }}
				activeOptions={{ exact: true }}
			>
				Workspace
			</Link>
			<Link
				to="/project/$id/branches"
				params={{ id: selectedProjectId }}
				className={styles.navLink}
				activeProps={{ className: styles.navLinkActive }}
			>
				Branches
			</Link>
		</nav>
	);
};

const TopBarActions: FC = () => {
	const [projectState, dispatchProjectState] = assert(use(ProjectStateContext));
	const { layout: layoutState } = projectState;

	return (
		<div className={styles.topBarActions}>
			<ShortcutButton
				binding={togglePreviewBinding}
				type="button"
				className={uiStyles.button}
				aria-pressed={isPreviewPanelVisible(layoutState)}
				onClick={() => dispatchProjectState({ _tag: "TogglePreview" })}
			>
				{togglePreviewBinding.description}
			</ShortcutButton>
			<ShortcutButton
				binding={toggleFullscreenPreviewBinding}
				type="button"
				className={uiStyles.button}
				aria-pressed={layoutState.isFullscreenPreviewOpen}
				onClick={() => dispatchProjectState({ _tag: "ToggleFullscreenPreview" })}
			>
				{toggleFullscreenPreviewBinding.description}
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

function RootLayout() {
	const [shortcutsBarPortalNode, setShortcutsBarPortalNode] = useState<HTMLElement | null>(null);
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});

	const content = (
		<ShortcutsBarPortalContext value={shortcutsBarPortalNode}>
			<main className={styles.layout}>
				<TopBar />
				<aside className={styles.sidebar}>
					<SidebarNav />
				</aside>
				<section className={styles.content}>
					<Outlet />
				</section>
				<footer className={styles.shortcutsBarFooter} ref={setShortcutsBarPortalNode} />
			</main>
		</ShortcutsBarPortalContext>
	);

	if (!projectMatch) return content;

	return <ProjectStateProvider key={projectMatch.params.id}>{content}</ProjectStateProvider>;
}

export const Route = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

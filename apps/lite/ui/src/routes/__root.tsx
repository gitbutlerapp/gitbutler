import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";
import { usePreviewFullscreen } from "../hooks/usePreviewFullscreen";
import { usePreviewVisible } from "../hooks/usePreviewVisible";
import { classes } from "#ui/classes.ts";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import uiStyles from "#ui/ui.module.css";
import styles from "./__root.module.css";
import { shortcutKeys } from "#ui/shortcuts.ts";

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

const ProjectPreviewActions: FC<{
	projectId: string;
}> = ({ projectId }) => {
	const [previewVisible, setPreviewVisible] = usePreviewVisible();
	const [, setShowPreviewFullscreen] = usePreviewFullscreen(projectId);

	return (
		<div className={styles.topBarPreviewActions}>
			<button
				type="button"
				className={classes(uiStyles.button)}
				aria-pressed={previewVisible}
				onClick={() => {
					setPreviewVisible((visible) => !visible);
				}}
			>
				Toggle preview ({shortcutKeys.togglePreview})
			</button>
			<button
				type="button"
				className={classes(uiStyles.button)}
				onClick={() => {
					setShowPreviewFullscreen(true);
				}}
			>
				Open fullscreen ({shortcutKeys.toggleFullscreenPreview})
			</button>
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
			{projectMatch && <ProjectPreviewActions projectId={projectMatch.params.id} />}
		</header>
	);
};

function RootLayout() {
	const [shortcutsBarPortalNode, setShortcutsBarPortalNode] = useState<HTMLElement | null>(null);

	return (
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
}

export const Route = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

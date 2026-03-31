import { useSuspenseQuery } from "@tanstack/react-query";
import { Link, Outlet, useMatch, useNavigate } from "@tanstack/react-router";
import { FC, useState } from "react";
import { QueryClient } from "@tanstack/react-query";
import { createRootRouteWithContext } from "@tanstack/react-router";
import { useFullscreenPreview } from "../hooks/useFullscreenPreview";
import { usePreviewPanel } from "../hooks/usePreviewPanel";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import {
	openFullscreenPreviewBinding,
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

const TopBarActions: FC<{
	projectId: string;
}> = ({ projectId }) => {
	const [previewPanel, setPreviewPanel] = usePreviewPanel();
	const [, setShowFullscreenPreview] = useFullscreenPreview(projectId);

	return (
		<div className={styles.topBarActions}>
			<ShortcutButton
				binding={togglePreviewBinding}
				type="button"
				className={uiStyles.button}
				aria-pressed={previewPanel}
				onClick={() => setPreviewPanel((visible) => !visible)}
			>
				{togglePreviewBinding.description}
			</ShortcutButton>
			<ShortcutButton
				binding={openFullscreenPreviewBinding}
				type="button"
				className={uiStyles.button}
				onClick={() => setShowFullscreenPreview(true)}
			>
				{openFullscreenPreviewBinding.description}
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
			{projectMatch && <TopBarActions projectId={projectMatch.params.id} />}
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

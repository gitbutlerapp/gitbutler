import { QueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { Link, Outlet, createRootRouteWithContext, useMatch } from "@tanstack/react-router";
import { FC } from "react";
import { usePreviewVisible } from "../hooks/usePreviewVisible";
import styles from "./root.module.css";
import { shortcutKeys } from "./shortcuts.ts";

export const lastOpenedProjectKey = "lastProject";

const ProjectSelect: FC = () => {
	const { data: projects } = useSuspenseQuery({
		queryKey: ["projects"],
		queryFn: () => window.lite.listProjects(),
	});
	const navigate = rootRoute.useNavigate();
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
					to: "/project/$id",
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
				to={"/project/$id"}
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

const TopBar: FC = () => {
	const projectMatch = useMatch({
		from: "/project/$id",
		shouldThrow: false,
	});
	const [previewVisible, setPreviewVisible] = usePreviewVisible();

	return (
		<header className={styles.topBar}>
			<ProjectSelect />
			{projectMatch && (
				<button
					type="button"
					className={styles.topBarPreviewToggle}
					aria-pressed={previewVisible}
					onClick={() => {
						setPreviewVisible((visible) => !visible);
					}}
				>
					{previewVisible ? <>Hide preview</> : <>Show preview</>} ({shortcutKeys.togglePreview})
				</button>
			)}
		</header>
	);
};

const RootLayout: FC = () => (
	<main className={styles.layout}>
		<TopBar />
		<aside className={styles.sidebar}>
			<SidebarNav />
		</aside>
		<section className={styles.content}>
			<Outlet />
		</section>
	</main>
);

interface RouteContext {
	queryClient: QueryClient;
}

export const rootRoute = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

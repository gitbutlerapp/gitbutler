import { Outlet, createRootRoute, createRoute, createRouter } from '@tanstack/react-router';
import type { ProjectForFrontend } from '@gitbutler/but-sdk';

function RootLayout(): React.JSX.Element {
	return (
		<main style={{ fontFamily: 'system-ui', margin: '2rem' }}>
			<h1>GitButler Lite</h1>
			<Outlet />
		</main>
	);
}

const rootRoute = createRootRoute({
	component: RootLayout
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: '/',
	component: HomePage,
	loader: async () => {
		const projects = await window.lite.listProjects();
		return { projects };
	}
});

function HomePage(): React.JSX.Element {
	const { projects } = indexRoute.useLoaderData();
	return (
		<section>
			<p>Electron + Vite + TanStack Router scaffold is ready.</p>
			<h2>Projects list</h2>
			<ProjectsList projects={projects} />
		</section>
	);
}

interface ProjectsListProps {
	projects: ProjectForFrontend[];
}

function ProjectsList(props: ProjectsListProps) {
	if (props.projects.length === 0) {
		return <p> no projects :(</p>;
	}
	return (
		<div>
			{props.projects.map((project) => (
				<p key={project.id}>{project.title}</p>
			))}
		</div>
	);
}

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({ routeTree });

declare module '@tanstack/react-router' {
	interface Register {
		router: typeof router;
	}
}

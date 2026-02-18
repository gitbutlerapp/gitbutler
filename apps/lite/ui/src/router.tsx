import { Outlet, createRootRoute, createRoute, createRouter } from '@tanstack/react-router';

function RootLayout(): React.JSX.Element {
	return (
		<main style={{ fontFamily: 'system-ui', margin: '2rem' }}>
			<h1>GitButler Lite</h1>
			<Outlet />
		</main>
	);
}

function HomePage(): React.JSX.Element {
	return (
		<section>
			<p>Electron + Vite + TanStack Router scaffold is ready.</p>
		</section>
	);
}

const rootRoute = createRootRoute({
	component: RootLayout
});

const indexRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: '/',
	component: HomePage
});

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({ routeTree });

declare module '@tanstack/react-router' {
	interface Register {
		router: typeof router;
	}
}

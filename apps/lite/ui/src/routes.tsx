import type { UrlQueryParams } from "#ui/cursor-url.ts";
import { handleProjectEvent } from "#ui/project-events.ts";
import { readLastOpenedProject, readLastPlace } from "#ui/project.ts";
import { IndexPage } from "#ui/routes/IndexPage.tsx";
import { HotkeysProvider } from "@tanstack/react-hotkeys";
import type { QueryClient } from "@tanstack/react-query";
import {
	createRootRouteWithContext,
	createRoute,
	notFound,
	Outlet,
	redirect,
} from "@tanstack/react-router";
import type { FC } from "react";
import styles from "./routes.module.css";

/**
 * Every route definition and the assembled tree, in one place. The routes/
 * directory mirrors the URL structure but holds only components; nothing
 * machine-reads its layout (no file-based codegen — this file is the whole
 * routing story). The tree is imported by main.tsx and passed into
 * createAppRouter; router.ts takes only the type, staying an import leaf
 * (see the cycle note there).
 */

interface RouteContext {
	queryClient: QueryClient;
}

// oxlint-disable-next-line react/only-export-components -- Route file hosts its layout by design.
const RootLayout: FC = () => (
	<HotkeysProvider>
		<div className={styles.dragRegion} />
		<main className={styles.content}>
			<Outlet />
		</main>
	</HotkeysProvider>
);

const rootRoute = createRootRouteWithContext<RouteContext>()({
	component: RootLayout,
});

const parseLastSearch = (search: string): Record<string, string> =>
	Object.fromEntries(new URLSearchParams(search));

const indexRoute = createRoute({
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

const projectRoute = createRoute({
	getParentRoute: () => rootRoute,
	path: "project/$id",
	remountDeps: ({ params }) => params.id,
	// Needed for `remountDeps` to work.
	component: () => <Outlet />,
	beforeLoad: async ({ matches, routeId, params }) => {
		// We don't want an index route.
		if (matches.at(-1)?.routeId === routeId) throw notFound();

		// The id decodes to a path, and URLs arrive from outside the app, so open
		// only projects it already knows about.
		const projects = await window.lite.listProjectsStateless();
		if (!projects.some((project) => project.id === params.id)) throw redirect({ to: "/" });
	},
	loader: async ({ params, context }) => {
		// Allow the route to render and handle failure via its queries.
		try {
			const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
				handleProjectEvent(event, params.id, context.queryClient),
			);
			return { subscriptionId };
		} catch {
			return { subscriptionId: undefined };
		}
	},
	onLeave: ({ loaderData }) => {
		if (loaderData?.subscriptionId !== undefined)
			void window.lite.watcherUnsubscribe(loaderData.subscriptionId);
	},
});

const str = (value: unknown): string | undefined =>
	typeof value === "string" && value !== "" ? value : undefined;

/**
 * The tree takes its workspace component instead of naming one: the app
 * mounts `Page`, the harness panel mounts its own surface, and every other
 * route definition is shared. Build one tree per process — the route
 * objects above are module singletons.
 */
export const createRouteTree = ({ workspace }: { workspace: FC }) => {
	const workspaceRoute = createRoute({
		getParentRoute: () => projectRoute,
		path: "workspace",
		component: workspace,
		// Total decoding: a param that fails its codec is absent, never an error,
		// so a corrupt or stale URL opens the page at defaults.
		validateSearch: (params: Record<string, unknown>): UrlQueryParams => {
			const page = str(params.page);
			const active = str(params.active);

			return {
				page: page === "upstream" || page === "branches" ? page : undefined,
				active: active === "uncommitted" ? active : undefined,
				applied: str(params.applied),
				uncommitted: str(params.uncommitted),
				unapplied: str(params.unapplied),
				upstream: str(params.upstream),
				files: str(params.files),
			};
		},
	});

	return rootRoute.addChildren([indexRoute, projectRoute.addChildren([workspaceRoute])]);
};

export type RouteTree = ReturnType<typeof createRouteTree>;

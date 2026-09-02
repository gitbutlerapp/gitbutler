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
import type { QueryKeyPrefix } from "./api/query-keys.ts";

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

/** What a project match carries that its watcher subscription needs. */
type ProjectMatch = { params: { id: string }; context: RouteContext };

/**
 * The window's one watcher subscription and the project it was requested
 * for, kept beside the route because the route is a module singleton.
 * Requests are chained, so the previous subscription is dropped once the
 * next is live however navigations overlap.
 */
let watched: { projectId?: string; subscription: Promise<string | undefined> } = {
	subscription: Promise.resolve(undefined),
};

/** Replace the subscription with one for the match's project, or with none. */
const watchProject = (match: ProjectMatch | null) => {
	const subscription = watched.subscription.then(async (previous) => {
		// The next one first: the host keeps a project's watcher running while
		// any subscription holds it, so re-opening the current project neither
		// restarts the watcher nor drops the events in between.
		let next: string | undefined;
		if (match !== null) {
			const { id } = match.params;
			next = await window.lite
				.watcherSubscribe(id, (event) => handleProjectEvent(event, id, match.context.queryClient))
				// Allow the route to render and handle failure via its queries.
				.catch(() => undefined);

			if (next !== undefined)
				void match.context.queryClient.invalidateQueries<QueryKeyPrefix>({ queryKey: [id] });
		}
		if (previous !== undefined) await window.lite.watcherUnsubscribe(previous).catch(() => false);
		return next;
	});
	watched = { projectId: match?.params.id, subscription };
	return subscription;
};

/** Hooks see the match on screen; they step in only when the loader's request was for another. */
const settleOn = (match: ProjectMatch) => {
	if (watched.projectId !== match.params.id) void watchProject(match);
};

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
	// Armed in the loader so the watcher is live before the page's queries
	// run. Switching projects reruns the loader without an `onLeave` in
	// between (the route id stays the same, only `$id` changes), and a switch
	// abandoned for the project already on screen reruns nothing at all, so
	// the hooks settle the subscription on whichever match is committed.
	loader: async (match) => {
		await watchProject(match);
	},
	onEnter: settleOn,
	onStay: settleOn,
	onLeave: () => void watchProject(null),
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

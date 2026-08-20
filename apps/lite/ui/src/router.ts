import { createRouter, type RouterHistory } from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";
import type { RouteTree } from "#ui/routes.tsx";

/* Every search param is a plain string (see cursor-url.ts), so the default
   JSON search serialization would only add quoting noise. Slashes and colons
   are legal in query values and carry most of our params' legibility. */
const stringifySearch = (search: Record<string, unknown>): string => {
	const parts = Object.entries(search).flatMap(([key, value]) =>
		typeof value === "string" && value !== ""
			? [`${key}=${encodeURIComponent(value).replaceAll("%2F", "/").replaceAll("%3A", ":")}`]
			: [],
	);
	return parts.length === 0 ? "" : `?${parts.join("&")}`;
};

const parseSearch = (searchStr: string): Record<string, unknown> =>
	Object.fromEntries(new URLSearchParams(searchStr));

/* The tree is passed in, not imported: importing its value closes a cycle back
   through use-cursor.ts, and workspace/route.tsx reads its page component at
   module scope, so HMR re-entry throws on the half-built binding. */
const buildRouter = (queryClient: QueryClient, routeTree: RouteTree, history?: RouterHistory) =>
	createRouter({ routeTree, context: { queryClient }, parseSearch, stringifySearch, history });

type AppRouter = ReturnType<typeof buildRouter>;

/**
 * `history` defaults to the browser's; the harness panel passes a memory
 * history, so the same router runs where no URL bar exists.
 */
export const createAppRouter = (
	queryClient: QueryClient,
	routeTree: RouteTree,
	history?: RouterHistory,
): AppRouter => {
	router = buildRouter(queryClient, routeTree, history);
	return router;
};

/**
 * Module-level once `createAppRouter` has run, so navigating is a plain call
 * from anywhere rather than a hook. Only test setups see it unset.
 */
export let router: AppRouter;

declare module "@tanstack/react-router" {
	interface Register {
		router: AppRouter;
	}
}

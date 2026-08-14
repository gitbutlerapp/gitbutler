import { routeTree } from "#ui/routeTree.ts";
import { createRouter } from "@tanstack/react-router";
import type { QueryClient } from "@tanstack/react-query";

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

const buildRouter = (queryClient: QueryClient) =>
	createRouter({ routeTree, context: { queryClient }, parseSearch, stringifySearch });

type AppRouter = ReturnType<typeof buildRouter>;

export const createAppRouter = (queryClient: QueryClient): AppRouter => {
	router = buildRouter(queryClient);
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

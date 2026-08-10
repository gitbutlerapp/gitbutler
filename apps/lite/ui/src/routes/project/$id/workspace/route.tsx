import { Route as projectRoute } from "#ui/routes/project/$id/route.tsx";
import type { UrlQueryParams } from "#ui/cursor-url.ts";
import { createRoute } from "@tanstack/react-router";
import { Route as WorkspacePageRoute } from "./WorkspacePage.tsx";

const str = (value: unknown): string | undefined =>
	typeof value === "string" && value !== "" ? value : undefined;

export const Route = createRoute({
	getParentRoute: () => projectRoute,
	path: "workspace",
	component: WorkspacePageRoute,
	// Total decoding: a param that fails its codec is absent, never an error,
	// so a corrupt or stale URL opens the page at defaults.
	validateSearch: (params: Record<string, unknown>): UrlQueryParams => {
		const page = str(params.page);
		const list = str(params.list);

		return {
			page: page === "upstream" || page === "branches" ? page : undefined,
			list: list === "uncommitted" ? list : undefined,
			stacks: str(params.stacks),
			uncommitted: str(params.uncommitted),
			branches: str(params.branches),
			upstream: str(params.upstream),
			files: str(params.files),
		};
	},
});

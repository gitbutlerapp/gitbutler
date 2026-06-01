import { buildActionEndpoints } from "$lib/actions/actionEndpoints";
import { buildBranchEndpoints } from "$lib/branches/branchEndpoints";
import { buildGitEndpoints } from "$lib/git/gitEndpoints";
import { buildIrcEndpoints } from "$lib/irc/ircEndpoints";
import { buildModeEndpoints } from "$lib/mode/modeEndpoints";
import { buildProjectEndpoints } from "$lib/project/projectEndpoints";
import { buildStackEndpoints } from "$lib/stacks/stackEndpoints";
import { tauriBaseQuery, type TauriBaseQueryFn } from "$lib/state/backendQuery";
import { butlerModule } from "$lib/state/butlerModule";
import { ReduxTag } from "$lib/state/tags";
import { buildUserEndpoints } from "$lib/user/userEndpoints";
import { buildWorktreeEndpoints } from "$lib/worktree/worktreeEndpoints";
import { buildCreateApi, coreModule } from "@reduxjs/toolkit/query";
import type { HookContext } from "$lib/state/context";
import type { EndpointBuilder } from "@reduxjs/toolkit/query";

export type BackendEndpointBuilder = EndpointBuilder<TauriBaseQueryFn, ReduxTag, "backend">;

/**
 * Creates the RTK Query API for the backend with all endpoints declared statically,
 * giving full TypeScript typing on the returned BackendApi.
 */
export function createBackendApi(ctx: HookContext) {
	return buildCreateApi(
		coreModule(),
		butlerModule(ctx),
	)({
		reducerPath: "backend",
		tagTypes: Object.values(ReduxTag),
		invalidationBehavior: "immediately",
		// Use RTK Query's default `keepUnusedDataFor` (60s). Setting it to
		// `0` schedules an immediate `setTimeout(0)` cleanup on each
		// query's fulfillment, which races with concurrent imperative
		// `fetch()` callers awaiting the same in-flight query — the
		// cleanup wins, the cache entry is gone before the second caller
		// reads it via `selectFromState`, and `unwrap()` silently returns
		// `undefined` instead of the data. This surfaced as
		// `TypeError: undefined is not an object (evaluating 't.url')` in
		// `UserService.getLoginUrl` (APP-JS-77K / APP-JS-77P). Freshness
		// for subscribed queries is already guaranteed by tag invalidation;
		// the default 60s eviction is purely a memory-hygiene fallback for
		// entries with no remaining subscribers.
		baseQuery: tauriBaseQuery,
		endpoints: (build) => ({
			...buildStackEndpoints(build),
			...buildBranchEndpoints(build),
			...buildWorktreeEndpoints(build),
			...buildGitEndpoints(build),
			...buildModeEndpoints(build),
			...buildProjectEndpoints(build),
			...buildActionEndpoints(build),
			...buildIrcEndpoints(build),
			...buildUserEndpoints(build),
		}),
	});
}

export type BackendApi = ReturnType<typeof createBackendApi>;

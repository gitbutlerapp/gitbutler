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
		keepUnusedDataFor: 0,
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
		}),
	});
}

export type BackendApi = ReturnType<typeof createBackendApi>;

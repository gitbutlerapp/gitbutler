import { apiProvides } from "@gitbutler/but-sdk/cache-tags";

/**
 * The project queries are the endpoints declaring `provides` in Rust — using a
 * name the backend doesn't declare is a type error. Keyed `[key, projectId,
 * ...]`; the fixed position is what lets an invalidation reach a whole query
 * root holding nothing but a project id.
 */
export type ProjectQueryKey = keyof typeof apiProvides;

// `Object.keys` erases key types; the record's keys are exactly these.
export const projectQueryKeys = Object.keys(apiProvides) as ReadonlyArray<ProjectQueryKey>;

/** Keyed without a project id, so no project event can invalidate them. */
type GlobalQueryKey =
	| "aiConfiguration"
	| "editors"
	| "terminals"
	| "forgeAccounts"
	| "userProfile"
	| "projects"
	| "guiSettings";

/**
 * Client state kept in the query cache, so nothing declares for them. `dryRun`
 * memoizes an imperative preview: its key carries the operation and changes it
 * was measured against, and nothing refreshes it in place.
 */
type LocalQueryKey =
	| "commitMessageDraft"
	| "dryRun"
	| "prMergeMethod"
	| "prDraft"
	| "projectAiSettings"
	| "reviewedFiles";

export type QueryKey = ProjectQueryKey | GlobalQueryKey | LocalQueryKey;

declare module "@tanstack/react-query" {
	interface Register {
		/**
		 * Every query key in the app starts with one of ours, so a typo is a type
		 * error wherever a key is written — building one, invalidating it, or
		 * reading it back — without each site having to say so.
		 */
		queryKey: readonly [QueryKey, ...ReadonlyArray<unknown>];
	}
}

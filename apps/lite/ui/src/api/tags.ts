/**
 * @file The cache-tag declarations, turned into invalidation.
 *
 * The backend describes its cache effects in three declarations: each read
 * endpoint provides tags, each mutation invalidates tags, and each watcher
 * event invalidates tags. This file connects them to the queries lite
 * actually caches, so "which caches to drop" is derived, never guessed.
 */

import { projectQueryKeys, type QueryKey } from "#ui/api/queries.ts";
import { apiInvalidates, apiProvides, type CacheTag } from "@gitbutler/but-sdk/cache-tags";
import type { QueryClient } from "@tanstack/react-query";

/**
 * Global queries by the tag they provide. The backend cannot know these:
 * they are what lite caches without a project scope, under keys of its own.
 */
const globalProviders: ReadonlyArray<[CacheTag, QueryKey]> = [
	["Projects", "projects"],
	["ForgeAccounts", "forgeAccounts"],
];

/**
 * Every query providing each tag, with the scope its key carries.
 *
 * The `apiProvides` index is the gate: a project query naming no declared
 * endpoint does not compile, so it has to be declared in Rust or become a
 * `LocalQueryKey`.
 */
const providers = new Map<CacheTag, Array<{ query: QueryKey; projectScoped: boolean }>>();
const provide = (tag: CacheTag, query: QueryKey, projectScoped: boolean) => {
	const queries = providers.get(tag);
	if (queries) queries.push({ query, projectScoped });
	else providers.set(tag, [{ query, projectScoped }]);
};
for (const query of projectQueryKeys)
	for (const tag of apiProvides[query]) provide(tag, query, true);
for (const [tag, query] of globalProviders) provide(tag, query, false);

/**
 * Drop every cache providing the given tags. Without a project id,
 * project-scoped queries are invalidated across all projects by key prefix.
 */
export const invalidateTags = (
	client: Pick<QueryClient, "invalidateQueries">,
	tags: ReadonlyArray<CacheTag>,
	projectId?: string,
): Promise<unknown> =>
	Promise.all(
		tags.flatMap((tag) =>
			(providers.get(tag) ?? []).map(({ query, projectScoped }) =>
				client.invalidateQueries({
					queryKey: projectScoped && projectId !== undefined ? [query, projectId] : [query],
				}),
			),
		),
	);

/**
 * The endpoint a mutation ran, recognized by the identity of its `mutationFn`.
 * A wrapped `mutationFn` is invisible here, so an endpoint that declares
 * `invalidates` must be passed to its mutation unwrapped.
 */
export const endpointOf = (mutationFn: unknown): string | undefined => {
	endpointByFn ??= new Map(Object.entries(window.lite).map(([name, fn]) => [fn, name]));
	return endpointByFn.get(mutationFn);
};
// Built on first use: `window.lite` only exists in the renderer, not in tests.
let endpointByFn: Map<unknown, string> | undefined;

/** The declarations by endpoint name, since an endpoint arrives as `unknown`. */
const declaredInvalidates = new Map<string, ReadonlyArray<CacheTag>>(
	Object.entries(apiInvalidates),
);

/**
 * Apply a finished mutation's declared invalidations. Wired once into the
 * query client's mutation cache; the endpoint comes from [`endpointOf`], so
 * any mutation whose `mutationFn` is a declared endpoint is covered.
 */
export const invalidateDeclared = (
	client: Pick<QueryClient, "invalidateQueries">,
	endpoint: string | undefined,
	variables: unknown,
): Promise<unknown> => {
	const tags = endpoint === undefined ? undefined : declaredInvalidates.get(endpoint);
	if (!tags) return Promise.resolve();
	const projectId =
		typeof variables === "object" &&
		variables !== null &&
		"projectId" in variables &&
		typeof variables.projectId === "string"
			? variables.projectId
			: undefined;
	return invalidateTags(client, tags, projectId);
};

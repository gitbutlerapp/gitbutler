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
const globalProviders: Partial<Record<CacheTag, ReadonlyArray<QueryKey>>> = {
	Projects: ["projects"],
	ForgeAccounts: ["forgeAccounts"],
};

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
for (const [tag, queries] of Object.entries(globalProviders) as Array<
	[CacheTag, ReadonlyArray<QueryKey>]
>)
	for (const query of queries) provide(tag, query, false);

/**
 * Drop every cache providing the given tags. Without a project id,
 * project-scoped queries are invalidated across all projects by key prefix.
 */
export const invalidateTags = (
	client: QueryClient,
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

/** A mutation endpoint that declared what it invalidates. */
export type DeclaredMutation = keyof typeof apiInvalidates & keyof typeof window.lite;

/**
 * The mutation options binding an endpoint to its declaration: the key names
 * the endpoint, so on success the endpoint's `invalidates` tags are applied
 * by the mutation cache. Spread it, overriding `mutationFn` when the call
 * needs wrapping.
 */
export const apiMutation = <Endpoint extends DeclaredMutation>(endpoint: Endpoint) => ({
	mutationKey: [endpoint] as const,
	mutationFn: window.lite[endpoint],
});

/** The declarations by endpoint name, since a mutation key arrives as `unknown`. */
const declaredInvalidates = new Map<string, ReadonlyArray<CacheTag>>(
	Object.entries(apiInvalidates),
);

/**
 * Apply a finished mutation's declared invalidations. Wired once into the
 * query client's mutation cache; mutations opt in by carrying their endpoint
 * as `mutationKey`, which `apiMutation` arranges.
 */
export const invalidateDeclared = (
	client: QueryClient,
	mutationKey: ReadonlyArray<unknown> | undefined,
	variables: unknown,
): Promise<unknown> => {
	const endpoint = mutationKey?.[0];
	const tags = typeof endpoint === "string" ? declaredInvalidates.get(endpoint) : undefined;
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

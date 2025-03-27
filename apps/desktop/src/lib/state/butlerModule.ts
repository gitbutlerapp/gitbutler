import {
	buildMutationHooks,
	buildQueryHooks,
	type UseMutationHookParams
} from '$lib/state/customHooks.svelte';
import { isMutationDefinition, isQueryDefinition } from '$lib/state/helpers';
import { type Reactive } from '@gitbutler/shared/storeUtils';
import {
	type BaseQueryFn,
	type EndpointDefinitions,
	type Api,
	type Module,
	type QueryArgFrom,
	type ResultTypeFrom,
	type QueryDefinition,
	type EndpointDefinition,
	type MutationDefinition,
	type QueryResultSelectorResult,
	type ApiModules,
	type MutationResultSelectorResult,
	type QueryActionCreatorResult,
	type StartQueryActionCreatorOptions
} from '@reduxjs/toolkit/query';
import type { tauriBaseQuery, TauriBaseQueryFn } from '$lib/state/backendQuery';
import type { HookContext } from '$lib/state/context';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';

/** Gives our module a namespace in the extended `ApiModules` interface. */
const butlerModuleName = Symbol();
type ButlerModule = typeof butlerModuleName;

/**
 * Extends the `ApiModules` interface with new definitions.
 *
 * These types are then combined with the core module definitions when
 * calling `buildCreateApi`, and the new methods become available on the
 * declared endpoints.
 */
declare module '@reduxjs/toolkit/query' {
	export interface ApiModules<
		// eslint-disable-next-line @typescript-eslint/no-unused-vars
		BaseQuery extends BaseQueryFn,
		Definitions extends EndpointDefinitions,
		// eslint-disable-next-line @typescript-eslint/no-unused-vars
		ReducerPath extends string,
		// eslint-disable-next-line @typescript-eslint/no-unused-vars
		TagTypes extends string
	> {
		[butlerModuleName]: {
			endpoints: {
				[K in keyof Definitions]: Definitions[K] extends CustomQuery<any>
					? QueryHooks<Definitions[K]>
					: Definitions[K] extends MutationDefinition<any, any, any, any>
						? MutationHooks<Definitions[K]>
						: Definitions[K];
			};
		};
	}
}

type CustomEndpoints<T> = {
	[x: string]: EndpointDefinition<any, any, any, any> & { [K in keyof T]: T[K] };
};

type ExtensionDefinitions = ApiModules<
	typeof tauriBaseQuery,
	CustomEndpoints<
		QueryHooks<CustomQuery<any>> & MutationHooks<MutationDefinition<any, any, any, any>>
	>,
	string,
	string
>[typeof butlerModuleName]['endpoints'];

/**
 * Injects custom methods onto endpoint definitions.
 *
 * This tries to recreate the experience of using RTK Query with the bundled
 * react hooks.
 */
export function butlerModule(ctx: HookContext): Module<ButlerModule> {
	return {
		name: butlerModuleName,

		init(api, _options, _context) {
			const anyApi = api as any as Api<any, ExtensionDefinitions, string, string, ButlerModule>;
			return {
				injectEndpoint(endpointName, definition) {
					const endpoint = anyApi.endpoints[endpointName]!; // Known to exist.
					if (isQueryDefinition(definition)) {
						const { fetch, useQuery, useQueryState, useQueries } = buildQueryHooks({
							endpointName,
							api,
							ctx
						});
						endpoint.fetch = fetch;
						endpoint.useQuery = useQuery;
						endpoint.useQueryState = useQueryState;
						endpoint.useQueries = useQueries;
					} else if (isMutationDefinition(definition)) {
						const { mutate, useMutation } = buildMutationHooks({
							endpointName,
							api,
							ctx
						});
						endpoint.useMutation = useMutation;
						endpoint.mutate = mutate;
					}
				}
			};
		}
	};
}

/**
 * Custom return type for the `QueryHooks` extensions.
 */
export type CustomResult<T extends QueryDefinition<any, any, any, any>> =
	QueryResultSelectorResult<T>;

/**
 * Custom return type for the `QueryHooks` extensions with refetch.
 */
export type CustomQueryResult<T extends QueryDefinition<any, any, any, any>> =
	QueryResultSelectorResult<T> & {
		refetch: () => Promise<void>;
	};

/**
 * Shorthand useful for service interfaces.
 */
export type SubscribeResult<T> = Reactive<QueryActionCreatorResult<CustomQuery<T>>>;
export type ReactiveResult<T> = Reactive<CustomResult<CustomQuery<T>>>;
export type AsyncResult<T> = Promise<CustomResult<CustomQuery<T>>>;

/**
 * Shorthand useful for service interfaces.
 */
export type FetchResult<T> = ResultTypeFrom<CustomQuery<T>>;

/**
 * It would be great to understand why it is necessary to set the args type
 * to `any`, anything else results in quite a number of type errors.
 */
type CustomArgs = any;

/**
 * A type representing transformation of results.
 *
 * Please note that there are two types of transformations, and that this is
 * secondary to `transformResponse` which is part of RTKQ. The purpose is for
 * having two is to be able to use `EntityAdapter`, and then select items
 * using the built-in selectors.
 */
type Transformer<T extends CustomQuery<any>> = (
	queryResult: ResultTypeFrom<T>,
	queryArgs: QueryArgFrom<T>
) => unknown;

/**
 * We need a default transformer because of some typescript weirdness that
 * happens when the options/transformer is omitted in a `useQuery` function
 * call. It works largely the same, except that you must explicitly specify
 * the transformer argument type to avoid inference errors.
 */
type DefaultTransformer<T extends CustomQuery<any>> = (
	queryResult: ResultTypeFrom<T>,
	queryArgs: QueryArgFrom<T>
) => ResultTypeFrom<T>;

/**
 * A custom defintion of our queries since it needs to be referenced in a few
 * different places.
 */
export type CustomQuery<T> = QueryDefinition<CustomArgs, TauriBaseQueryFn, string, T>;

/** Options for queries. */
export type QueryOptions = StartQueryActionCreatorOptions & { forceRefetch?: boolean };

/**
 * Declaration of custom methods for queries.
 */
type QueryHooks<D extends CustomQuery<unknown>> = {
	/** Execute query and return results. */
	subscribe: (
		args: QueryArgFrom<D>,
		options: QueryOptions
	) => QueryActionCreatorResult<CustomQuery<ResultTypeFrom<D>>>;
	/** Fetch as promise, non-reactive. */
	fetch: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T; forceRefetch?: boolean }
	) => Promise<
		CustomResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
	/** Execute query and return results. */
	useQuery: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) => Reactive<
		CustomQueryResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
	/** Execute query on existing state. */
	useQueryState: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T }
	) => Reactive<
		CustomResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
	useQueries: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		queryArgs: QueryArgFrom<D>[],
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) => Reactive<
		CustomResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>[]
	>;
};

export type CustomMutationResult<Definition extends MutationDefinition<any, any, string, any>> =
	Prettify<MutationResultSelectorResult<Definition>>;

type CustomMutation<Definition extends MutationDefinition<any, any, string, any>> = readonly [
	/**
	 * Trigger the mutation with the given arguments.
	 *
	 * If awaited, the result will contain the mutation result.
	 */
	(args: QueryArgFrom<Definition>) => Promise<Prettify<ResultTypeFrom<Definition>>>,
	/**
	 * The reactive state of the mutation.
	 *
	 * This contains the result (if any yet) of the mutation plus additional information about its state.
	 */
	Reactive<CustomMutationResult<Definition>>,
	/**
	 * A method to reset the hook back to its original state and remove the current result from the cache.
	 */
	() => void
];

type Result<A> = {
	data?: A;
	error?: unknown;
};

/**
 * Declaration of custom methods for mutations.
 */
type MutationHooks<Definition extends MutationDefinition<unknown, any, string, unknown>> = {
	/** Execute query and return results. */
	useMutation: (params?: UseMutationHookParams<Definition>) => Prettify<CustomMutation<Definition>>;
	mutate: (args: QueryArgFrom<Definition>) => Promise<Result<Prettify<ResultTypeFrom<Definition>>>>;
};

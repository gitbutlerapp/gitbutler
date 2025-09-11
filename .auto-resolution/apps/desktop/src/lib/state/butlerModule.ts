import {
	buildMutationHook,
	buildQueryHooks,
	type MutationHook
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
	type QueryActionCreatorResult,
	type StartQueryActionCreatorOptions
} from '@reduxjs/toolkit/query';
import type { TauriBaseQueryFn } from '$lib/state/backendQuery';
import type { HookContext } from '$lib/state/context';
import type { Readable } from 'svelte/store';

/** Gives our module a namespace in the extended `ApiModules` interface. */
export const butlerModuleName = Symbol();
type ButlerModule = typeof butlerModuleName;

export type ExtraOptions = {
	// I have tried in vain to make this property required, but getting
	// types working correctly for `extraOptions` would be sick.
	// TODO: Find a way to make `actionName` required.
	actionName?: string;
};

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
						? MutationHook<Definitions[K]>
						: Definitions[K];
			};
		};
	}
}

type CustomEndpoints<T> = {
	[x: string]: EndpointDefinition<any, any, any, any> & { [K in keyof T]: T[K] };
};

function extractActionName(extraOptions: unknown): string | undefined {
	if (
		extraOptions &&
		typeof extraOptions === 'object' &&
		'actionName' in extraOptions &&
		typeof extraOptions.actionName === 'string'
	) {
		return extraOptions.actionName;
	}
	return undefined;
}

function extractCommand(extraOptions: unknown): string | undefined {
	if (
		extraOptions &&
		typeof extraOptions === 'object' &&
		'command' in extraOptions &&
		typeof extraOptions.command === 'string'
	) {
		return extraOptions.command;
	}
	return undefined;
}

export type ExtensionDefinitions = ApiModules<
	TauriBaseQueryFn,
	CustomEndpoints<
		QueryHooks<CustomQuery<any>> & MutationHook<MutationDefinition<any, TauriBaseQueryFn, any, any>>
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
			const anyApi = api as any as Api<
				TauriBaseQueryFn,
				ExtensionDefinitions,
				string,
				string,
				ButlerModule
			>;
			return {
				injectEndpoint(endpointName, definition) {
					const endpoint = anyApi.endpoints[endpointName]!; // Known to exist.
					if (isQueryDefinition(definition)) {
						const command = extractCommand(definition.extraOptions);
						const actionName = extractActionName(definition.extraOptions);
						const { fetch, useQuery, useQueryState, useQueries, useQueryTimeStamp } =
							buildQueryHooks({
								endpointName,
								command,
								actionName,
								api,
								ctx
							});
						endpoint.fetch = fetch;
						endpoint.useQuery = useQuery;
						endpoint.useQueryState = useQueryState;
						endpoint.useQueries = useQueries;
						endpoint.useQueryTimeStamp = useQueryTimeStamp;
					} else if (isMutationDefinition(definition)) {
						const actionName = extractActionName(definition.extraOptions);
						const command = extractCommand(definition.extraOptions);

						const { mutate, useMutation } = buildMutationHook({
							endpointName,
							actionName,
							command,
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
export type ReactiveResult<T> = Reactive<CustomResult<CustomQuery<T>>>;
export type AsyncResult<T> = Promise<CustomResult<CustomQuery<T>>>;

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
	) => Promise<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>;
	/** Execute query and return results. */
	useQuery: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) => Reactive<
		CustomQueryResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
	useQueryStore: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) => Readable<
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
	useQueryTimeStamp: (queryArgs: QueryArgFrom<D>) => Reactive<number | undefined>;
};

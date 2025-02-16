import { buildMutationHooks, buildQueryHooks } from './customHooks.svelte';
import { isMutationDefinition, isQueryDefinition } from './helpers';
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
	type MutationActionCreatorResult
} from '@reduxjs/toolkit/query';
import type { tauriBaseQuery } from './backendQuery';
import type { HookContext } from './context';

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
	BaseQueryFn,
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
						const { useQuery, useQueryState } = buildQueryHooks({
							endpointName,
							api,
							ctx
						});
						endpoint.useQuery = useQuery;
						endpoint.useQueryState = useQueryState;
					} else if (isMutationDefinition(definition)) {
						const { useMutation } = buildMutationHooks({
							endpointName,
							api,
							ctx
						});
						endpoint.useMutation = useMutation;
					}
				}
			};
		}
	};
}

/**
 * Custom return type for the `QueryHooks` extensions.
 */
type CustomResult<T extends QueryDefinition<any, any, any, any>> = QueryResultSelectorResult<T> & {
	/**
	 * Allows using the result from one query in the arguments to another.
	 *
	 * Example: ```
	 *   const result = $derived(
	 *     someService
	 *       .getData(someId).current
	 *       .andThen((data) => anotherService.getData(data.id)).current
	 *   );
	 * ```
	 */
	andThen<S extends (arg1: ResultTypeFrom<T>) => any>(fn: S): ReturnType<S>;
};

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
type Transformer<T extends CustomQuery<any>> = (arg: ResultTypeFrom<T>) => unknown;

/**
 * We need a default transformer because of some typescript weirdness that
 * happens when the options/transformer is omitted in a `useQuery` function
 * call. It works largely the same, except that you must explicitly specify
 * the transformer argument type to avoid inference errors.
 */
type DefaultTransformer<T extends CustomQuery<any>> = (arg: ResultTypeFrom<T>) => ResultTypeFrom<T>;

/**
 * A custom defintion of our queries since it needs to be referenced in a few
 * different places.
 */
export type CustomQuery<T> = QueryDefinition<CustomArgs, typeof tauriBaseQuery, string, T>;

/**
 * Declaration of custom methods for queries.
 */
type QueryHooks<D extends CustomQuery<unknown>> = {
	/** Execute query and return results. */
	useQuery: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T }
	) => Reactive<
		CustomResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
	/** Execute query on existing state. */
	useQueryState: <T extends Transformer<D> | undefined = DefaultTransformer<D>>(
		args: QueryArgFrom<D>,
		options?: { transform?: T }
	) => Reactive<
		CustomResult<CustomQuery<T extends Transformer<D> ? ReturnType<T> : ResultTypeFrom<D>>>
	>;
};

/**
 * Declaration of custom methods for mutations.
 */
type MutationHooks<Definition extends MutationDefinition<any, any, string, any>> = {
	/** Execute query and return results. */
	useMutation: (
		args: QueryArgFrom<Definition>
	) => MutationActionCreatorResult<
		MutationDefinition<CustomArgs, BaseQueryFn, string, ResultTypeFrom<Definition>>
	>;
};

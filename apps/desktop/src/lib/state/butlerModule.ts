import { buildMutationHooks, buildQueryHooks, type HookContext } from './customHooks.svelte';
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
				[K in keyof Definitions]: Definitions[K] extends QueryDefinition<any, any, any, any>
					? QueryHooks<Definitions[K]>
					: Definitions[K] extends MutationDefinition<any, any, any, any>
						? MutationHooks<Definitions[K]>
						: Definitions[K];
			};
		};
	}
}

type CustomDefinition<T> = {
	[x: string]: EndpointDefinition<any, any, any, any> & { [K in keyof T]: T[K] };
};

type ExtensionDefinitions = ApiModules<
	BaseQueryFn,
	CustomDefinition<
		QueryHooks<QueryDefinition<any, any, any, any>> &
			MutationHooks<MutationDefinition<any, any, any, any>>
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
 * Declaration of custom methods for queries.
 */
type QueryHooks<Definition extends QueryDefinition<any, any, any, any>> = {
	/** Execute query and return results. */
	useQuery: (
		args: QueryArgFrom<Definition>
	) => Reactive<
		QueryResultSelectorResult<
			Extract<Definition, QueryDefinition<any, any, any, ResultTypeFrom<Definition>>>
		>
	>;
	/** Execute query on existing state. */
	useQueryState: <T extends (args: ResultTypeFrom<Definition>) => any>(
		args: QueryArgFrom<Definition>,
		/** Optional transformation of the result.  */
		transform?: T
	) => Reactive<QueryResultSelectorResult<QueryDefinition<any, any, any, ReturnType<T>>>>;
};

/**
 * Declaration of custom methods for mutations.
 */
type MutationHooks<Definition extends MutationDefinition<any, any, string, any>> = {
	/** Execute query and return results. */
	useMutation: (
		args: QueryArgFrom<Definition>
	) => MutationActionCreatorResult<
		MutationDefinition<unknown, BaseQueryFn, string, ResultTypeFrom<Definition>>
	>;
};

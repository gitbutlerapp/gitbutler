import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type CoreModule,
	type EndpointDefinitions,
	type MutationDefinition,
	type QueryDefinition,
	type RootState
} from '@reduxjs/toolkit/query';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';

/**
 *	The api is necessary to create the store, so we need to provide
 *	a way for them to access state and dispatch. In react it's possible
 *	to use the application context since it is available to events
 *	fired by components, while Svelte requires `getContext` only be
 *	used during component initialization.
 */
export type HookContext = {
	/** Without the nested function we get looping reactivity.  */
	getState: () => () => RootState<any, any, any>;
	getDispatch: () => ThunkDispatch<any, any, UnknownAction>;
};

/**
 * Returns implementations for custom endpoint methods defined in `ButlerModule`.
 */
export function buildQueryHooks<Definitions extends EndpointDefinitions>({
	api,
	endpointName,
	ctx: { getState, getDispatch }
}: {
	api: Api<any, Definitions, any, any, CoreModule>;
	endpointName: string;
	ctx: HookContext;
}) {
	const endpoint = api.endpoints[endpointName]!;

	const { initiate, select } = endpoint as ApiEndpointQuery<
		QueryDefinition<any, any, any, any, any>,
		Definitions
	>;

	function useQuery(queryArg: unknown) {
		const dispatch = getDispatch();
		$effect(() => {
			const { unsubscribe } = dispatch(initiate(queryArg));
			return unsubscribe;
		});
		const result = $derived(useQueryState(queryArg));
		return result;
	}

	function useQueryState<T extends (arg: any) => any>(queryArg: unknown, transform?: T) {
		const state = getState();
		const selector = $derived(select(queryArg));
		const result = $derived(selector(state()));
		return {
			get current() {
				const data = transform ? transform(result.data) : result.data;
				function andThen(fn: (arg: any) => any) {
					if (data) {
						return fn(data);
					} else {
						return result;
					}
				}
				return {
					...result,
					data,
					andThen
				};
			}
		};
	}

	return {
		useQuery,
		useQueryState
	};
}

/**
 * Returns implementations for custom endpoint methods defined in `ButlerModule`.
 */
export function buildMutationHooks<Definitions extends EndpointDefinitions>({
	api,
	endpointName,
	ctx: { getDispatch }
}: {
	api: Api<any, Definitions, any, any, CoreModule>;
	endpointName: string;
	ctx: HookContext;
}) {
	const endpoint = api.endpoints[endpointName]!;

	const { initiate } = endpoint as ApiEndpointMutation<
		MutationDefinition<any, any, any, any, any>,
		Definitions
	>;

	/**
	 * TODO: Introduce similar functionality to useMutation in the react hooks.
	 */
	// eslint-disable-next-line @typescript-eslint/promise-function-async
	function useMutation(queryArg: unknown) {
		const dispatch = getDispatch();
		const result = dispatch(initiate(queryArg));
		return result;
	}

	return {
		useMutation
	};
}

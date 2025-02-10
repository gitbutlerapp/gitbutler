import { reactive } from '@gitbutler/shared/storeUtils';
import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type BaseQueryFn,
	type CoreModule,
	type EndpointDefinitions,
	type MutationDefinition,
	type QueryDefinition,
	type RootState
} from '@reduxjs/toolkit/query';
import type { HookContext } from './context';

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
	const state = getState() as any as () => RootState<any, any, any>;

	const { initiate, select } = endpoint as ApiEndpointQuery<
		QueryDefinition<unknown, BaseQueryFn, string, any>,
		Definitions
	>;

	function useQuery<T extends (arg: any) => any>(queryArg: unknown, options?: { transform?: T }) {
		const dispatch = getDispatch();
		$effect(() => {
			const { unsubscribe } = dispatch(initiate(queryArg));
			return unsubscribe;
		});
		const result = $derived(useQueryState(queryArg, options));
		return result;
	}

	function useQueryState<T extends (arg: any) => any>(
		queryArg: unknown,
		options?: { transform?: T }
	) {
		const selector = $derived(select(queryArg));
		const result = $derived(selector(state()));
		const output = $derived.by(() => {
			let data = result.data;
			if (options?.transform && data) {
				data = options.transform(data);
			}
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
		});
		return reactive(() => output);
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

import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type CoreModule,
	type EndpointDefinitions,
	type MutationActionCreatorResult,
	type MutationDefinition,
	type QueryActionCreatorResult,
	type ResultTypeFrom,
	type RootState
} from '@reduxjs/toolkit/query';
import type { CustomQuery } from './butlerModule';
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

	const { initiate, select } = endpoint as ApiEndpointQuery<CustomQuery<any>, Definitions>;

	async function fetch<T extends (arg: any) => any>(
		queryArg: unknown,
		options?: { transform?: T }
	) {
		const dispatch = getDispatch();
		const result = await dispatch(initiate(queryArg, { forceRefetch: true }));
		let data = result.data;
		if (options?.transform && data) {
			data = options.transform(data);
		}
		return data;
	}

	function useQuery<T extends (arg: any) => any>(
		queryArg: unknown,
		options?: { transform?: T; subscribe?: Reactive<{ pollingInterval?: number }> }
	) {
		const dispatch = getDispatch();
		let subscription: QueryActionCreatorResult<any>;
		$effect(() => {
			subscription = dispatch(initiate(queryArg));
			return subscription.unsubscribe;
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
		fetch,
		useQuery,
		useQueryState
	};
}

export type UseMutationHookParams<Definition extends MutationDefinition<any, any, string, any>> = {
	fixedCacheKey?: string;
	sideEffect?: (data: ResultTypeFrom<Definition>) => void;
	onError?: (error: unknown) => void;
};

/**
 * Returns implementations for custom endpoint methods defined in `ButlerModule`.
 */
export function buildMutationHooks<Definitions extends EndpointDefinitions>({
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

	const { initiate, select } = endpoint as ApiEndpointMutation<
		MutationDefinition<any, any, any, any, any>,
		Definitions
	>;

	/**
	 * Use mutation hook.
	 *
	 * @returns An object containing the reactive result of the mutation, a function to trigger the mutation and another one
	 * to reset it.
	 *
	 * Replicate the behavior of `useMutation` from RTK Query.
	 * @see: https://github.com/reduxjs/redux-toolkit/blob/637b0cad2b227079ccd0c5a3073c09ace6d8759e/packages/toolkit/src/query/react/buildHooks.ts#L867-L935
	 */
	function useMutation(
		params?: UseMutationHookParams<MutationDefinition<any, any, any, any, any>>
	) {
		const { fixedCacheKey, sideEffect, onError } = params || {};
		const dispatch = getDispatch();

		let promise =
			$state<MutationActionCreatorResult<MutationDefinition<any, any, any, any, any>>>();

		async function triggerMutation(queryArg: unknown) {
			const dispatchResult = dispatch(initiate(queryArg, { fixedCacheKey }));
			promise = dispatchResult;
			const result = await promise;

			if (result.data) {
				sideEffect?.(result.data);
			}

			if (result.error && !onError) {
				throw result.error;
			}

			if (result.error && onError) {
				onError(result.error);
			}

			return result.data;
		}

		function reset() {
			const requestId = promise?.requestId;
			if (promise) {
				promise = undefined;
			}
			if (fixedCacheKey) {
				dispatch(api.internalActions.removeMutationResult({ requestId, fixedCacheKey }));
			}
		}

		const selector = $derived(select({ requestId: promise?.requestId, fixedCacheKey }));
		const result = $derived(selector(state()));

		$effect(() => {
			return () => {
				if (promise && !promise.arg.fixedCacheKey) {
					// If there is no fixedCacheKey,
					// reset the mutation subscription on unmount.
					promise.reset();
				}
			};
		});

		return [triggerMutation, reactive(() => result), reset] as const;
	}

	return {
		useMutation
	};
}

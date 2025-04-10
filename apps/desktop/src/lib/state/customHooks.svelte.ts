import { reactive } from '@gitbutler/shared/storeUtils';
import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type CoreModule,
	type EndpointDefinitions,
	type MutationActionCreatorResult,
	type MutationDefinition,
	type QueryActionCreatorResult,
	type QueryArgFrom,
	type ResultTypeFrom,
	type RootState,
	type StartQueryActionCreatorOptions
} from '@reduxjs/toolkit/query';
import type { TauriCommandError } from '$lib/state/backendQuery';
import type { CustomQuery } from '$lib/state/butlerModule';
import type { HookContext } from '$lib/state/context';

type TranformerFn = (data: any, args: any) => any;

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

	async function fetch<T extends TranformerFn>(
		queryArg: unknown,
		options?: { transform?: T; forceRefetch?: boolean }
	) {
		const dispatch = getDispatch();
		const result = await dispatch(
			initiate(queryArg, {
				subscribe: false,
				forceRefetch: options?.forceRefetch
			})
		);
		let data = result.data;
		if (options?.transform && data) {
			data = options.transform(data, queryArg);
			return {
				...result,
				data
			};
		}
		return result;
	}

	function useQuery<T extends TranformerFn>(
		queryArg: unknown,
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) {
		const dispatch = getDispatch();
		let subscription: QueryActionCreatorResult<any> | undefined;
		$effect(() => {
			subscription = dispatch(
				initiate(queryArg, {
					subscribe: options?.subscribe,
					subscriptionOptions: options?.subscriptionOptions,
					forceRefetch: options?.forceRefetch
				})
			);
			return () => {
				subscription?.unsubscribe();
			};
		});

		async function refetch() {
			await subscription?.refetch();
		}

		const selector = $derived(select(queryArg));
		const result = $derived(selector(state()));
		const output = $derived.by(() => {
			let data = result.data;
			if (options?.transform && data) {
				data = options.transform(data, queryArg);
			}
			return {
				...result,
				refetch,
				data
			};
		});

		return reactive(() => output);
	}

	function useQueries<T extends TranformerFn>(
		queryArgs: unknown[],
		options?: { transform?: T } & StartQueryActionCreatorOptions
	) {
		const dispatch = getDispatch();
		let subscriptions: QueryActionCreatorResult<any>[];
		$effect(() => {
			// eslint-disable-next-line @typescript-eslint/promise-function-async
			subscriptions = queryArgs.map((queryArg) =>
				dispatch(
					initiate(queryArg, {
						subscribe: options?.subscribe,
						subscriptionOptions: options?.subscriptionOptions,
						forceRefetch: options?.forceRefetch
					})
				)
			);
			return () => {
				subscriptions.forEach((subscription) => subscription.unsubscribe());
			};
		});

		const results = queryArgs.map((queryArg) => {
			const selector = $derived(select(queryArg));
			const result = $derived(selector(state()));
			const output = $derived.by(() => {
				let data = result.data;
				if (options?.transform && data) {
					data = options.transform(data, queryArg);
				}
				return {
					...result,
					data
				};
			});
			return output;
		});
		return reactive(() => results);
	}

	function useQueryState<T extends TranformerFn>(queryArg: unknown, options?: { transform?: T }) {
		const selector = $derived(select(queryArg));
		const result = $derived(selector(state()));
		const output = $derived.by(() => {
			let data = result.data;
			if (options?.transform && data) {
				data = options.transform(data, queryArg);
			}
			return {
				...result,
				data
			};
		});
		return reactive(() => output);
	}

	return {
		fetch,
		useQuery,
		useQueryState,
		useQueries
	};
}

export type UseMutationHookParams<Definition extends MutationDefinition<any, any, string, any>> = {
	fixedCacheKey?: string;
	preEffect?: (queryArgs: QueryArgFrom<Definition>) => void;
	sideEffect?: (data: ResultTypeFrom<Definition>, queryArgs: QueryArgFrom<Definition>) => void;
	onError?: (error: TauriCommandError, queryArgs: QueryArgFrom<Definition>) => void;
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

	async function mutate(
		queryArg: unknown,
		options?: UseMutationHookParams<MutationDefinition<any, any, any, any, any>>
	) {
		const dispatch = getDispatch();
		const { fixedCacheKey, sideEffect, preEffect, onError } = options ?? {};
		preEffect?.(queryArg);
		const result = await dispatch(initiate(queryArg, { fixedCacheKey }));
		if (!result.error) {
			sideEffect?.(result.data, queryArg);
		}

		if (result.error && onError) {
			onError(result.error, queryArg);
		}

		return result;
	}

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
		const { fixedCacheKey, preEffect, sideEffect, onError } = params || {};
		const dispatch = getDispatch();

		let promise =
			$state<MutationActionCreatorResult<MutationDefinition<any, any, any, any, any>>>();

		async function triggerMutation(queryArg: unknown) {
			preEffect?.(queryArg);
			const dispatchResult = dispatch(initiate(queryArg, { fixedCacheKey }));
			promise = dispatchResult;
			const result = await promise;

			if (!result.error) {
				sideEffect?.(result.data, queryArg);
			}

			if (result.error && onError) {
				onError(result.error, queryArg);
			}

			return result;
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
		mutate,
		useMutation
	};
}

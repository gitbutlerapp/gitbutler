import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type CoreModule,
	type EndpointDefinitions,
	type MutationActionCreatorResult,
	type MutationDefinition,
	type MutationResultSelectorResult,
	type QueryActionCreatorResult,
	type QueryArgFrom,
	type ResultTypeFrom,
	type RootState,
	type StartQueryActionCreatorOptions
} from '@reduxjs/toolkit/query';
import type { CustomQuery } from '$lib/state/butlerModule';
import type { HookContext } from '$lib/state/context';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';

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

	function useQueryTimeStamp(queryArg: unknown) {
		const selector = $derived(select(queryArg));
		const result = $derived(selector(state()));
		return reactive(() => result.startedTimeStamp);
	}

	return {
		fetch,
		useQuery,
		useQueryState,
		useQueries,
		useQueryTimeStamp
	};
}

export type UseMutationHookParams<Definition extends MutationDefinition<any, any, string, any>> = {
	fixedCacheKey?: string;
	/**
	 * A callback to be called when the trigger has been pulled, but before the mutation has been dispatched.
	 */
	preEffect?: (queryArgs: QueryArgFrom<Definition>) => void;
	/**
	 * A callback to be called when the mutation is successful.
	 */
	sideEffect?: (data: ResultTypeFrom<Definition>, queryArgs: QueryArgFrom<Definition>) => void;
	/**
	 * A callback to be called when the mutation fails.
	 *
	 * This does not stop the error from being thrown, but allows you to add a side effect depending on the error.
	 */
	onError?: (error: TauriCommandError, queryArgs: QueryArgFrom<Definition>) => void;
};

export type CustomMutationResult<Definition extends MutationDefinition<any, any, string, any>> =
	Prettify<MutationResultSelectorResult<Definition>>;

type CustomMutation<Definition extends MutationDefinition<any, any, string, any>> = readonly [
	/**
	 * Trigger the mutation with the given arguments.
	 *
	 * If awaited, the result will contain the mutation result.
	 */
	(args: QueryArgFrom<Definition>) => Promise<ResultTypeFrom<Definition>>,
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

/**
 * Declaration of custom methods for mutations.
 */
export interface MutationHooks<
	Definition extends MutationDefinition<unknown, any, string, unknown>
> {
	/**
	 * Mutation hook.
	 *
	 * Returns a function to trigger the mutation, a reactive state of the mutation and a function to reset it.
	 * */
	useMutation: (params?: UseMutationHookParams<Definition>) => Prettify<CustomMutation<Definition>>;
	/**
	 * Execute query and return results.
	 */
	mutate(
		args: QueryArgFrom<Definition>,
		options?: UseMutationHookParams<Definition>
	): Promise<ResultTypeFrom<Definition>>;
}

/**
 * Returns implementations for custom endpoint methods defined in `ButlerModule`.
 */
export function buildMutationHooks<
	Definitions extends EndpointDefinitions,
	D extends MutationDefinition<unknown, any, string, unknown>
>({
	api,
	endpointName,
	ctx: { getState, getDispatch }
}: {
	api: Api<any, Definitions, any, any, CoreModule>;
	endpointName: string;
	ctx: HookContext;
}): MutationHooks<D> {
	const endpoint = api.endpoints[endpointName]!;
	const state = getState() as any as () => RootState<any, any, any>;

	const { initiate, select } = endpoint as unknown as ApiEndpointMutation<D, Definitions>;
	async function mutate(queryArg: QueryArgFrom<D>, options?: UseMutationHookParams<D>) {
		const dispatch = getDispatch();
		const { fixedCacheKey, sideEffect, preEffect, onError } = options ?? {};
		preEffect?.(queryArg);
		const dispatchResult = dispatch(initiate(queryArg, { fixedCacheKey }));
		try {
			const result = await dispatchResult.unwrap();
			sideEffect?.(result, queryArg);
			return result;
		} catch (error: unknown) {
			if (onError && isTauriCommandError(error)) {
				onError(error, queryArg);
			}
			throw error;
		}
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
	function useMutation(params?: UseMutationHookParams<D>) {
		const { fixedCacheKey, preEffect, sideEffect, onError } = params || {};
		const dispatch = getDispatch();

		let promise = $state<MutationActionCreatorResult<D>>();

		async function triggerMutation(queryArg: QueryArgFrom<D>) {
			preEffect?.(queryArg);
			promise = dispatch(initiate(queryArg, { fixedCacheKey }));
			try {
				const result = await promise.unwrap();
				sideEffect?.(result, queryArg);
				return result;
			} catch (error: unknown) {
				if (onError && isTauriCommandError(error)) {
					onError(error, queryArg);
				}
				throw error;
			}
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

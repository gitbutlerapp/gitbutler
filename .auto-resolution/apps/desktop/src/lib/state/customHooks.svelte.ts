import { reactive, type Reactive } from '@gitbutler/shared/storeUtils';
import {
	type Api,
	type ApiEndpointMutation,
	type ApiEndpointQuery,
	type CoreModule,
	type EndpointDefinitions,
	type MutationDefinition,
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
			console.log('INITIATING');
			subscription = dispatch(
				initiate(queryArg, { subscriptionOptions: { pollingInterval: 5000 } })
			);
			return subscription.unsubscribe;
		});
		$effect(() => {
			console.log('hello world', subscription);
		});

		$effect(() => {
			// console.log(options);
			// console.log(options?.subscribe?.current.pollingInterval);
			// console.log('subscription', subscription);
			// if (options?.subscribe?.current) {
			// 	console.log('updating subscription', options?.subscribe);
			// 	subscription.updateSubscriptionOptions({
			// 		pollingInterval: options?.subscribe?.current.pollingInterval
			// 	});
			// }
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

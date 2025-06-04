import {
	QueryStatus,
	type EndpointDefinition,
	type MutationDefinition,
	type QueryDefinition
} from '@reduxjs/toolkit/query';
import type { CustomQuery, CustomResult } from '$lib/state/butlerModule';

/** Copied from redux-toolkit, it isn't exported. */
export function isQueryDefinition(
	e: EndpointDefinition<unknown, any, string, unknown>
): e is QueryDefinition<unknown, any, string, unknown> {
	return e.type === 'query';
}

/** Copied from redux-toolkit, it isn't exported. */
export function isMutationDefinition(
	e: EndpointDefinition<unknown, any, string, unknown>
): e is MutationDefinition<unknown, any, string, unknown> {
	return e.type === 'mutation';
}

/**
 * Note: Undefined is excluded from the return type of the query results.
 *
 * It is difficult to know if the return type can be undefined since we only
 * have results, not the queries themselves. So when combining results they
 * need to use `null` rather than undefined for any optional return value.
 *
 * An example of where this becomes relevant is when transforming the result
 * using selectNth.
 */
export function combineResults<T extends [...CustomResult<any>[]]>(
	...results: T
): CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>> {
	if (results.length === 0) {
		return {
			status: QueryStatus.uninitialized,
			error: undefined,
			data: undefined
		} as CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>>;
	}

	const data = results.every((r) => r.data !== undefined) ? results.map((r) => r.data) : undefined;
	const error = results.find((r) => r.error)?.error;
	const status = (results.find((r) => r.status === QueryStatus.rejected) ||
		results.find((r) => r.status === QueryStatus.uninitialized) ||
		results.find((r) => r.status === QueryStatus.pending) ||
		results.find((r) => r.status === QueryStatus.fulfilled))!.status;
	return {
		status,
		error,
		data
	} as CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>>;
}

/**
 * Transforms the `data` property of a successful Redux query result using the provided transformation function.
 *
 * If the query result is successful (`isSuccess` is true), applies the `transform` function to the `data` property
 * and returns a new result object with the transformed data. If not successful, returns the result with `data` set to `undefined`.
 *
 * @typeParam Result - The type of the original result data.
 * @typeParam NewResult - The type of the transformed result data.
 * @typeParam T - The type of the input query result, extending `CustomResult<QueryDefinition<any, any, any, Result>>`.
 * @param queryResult - The original Redux query result object.
 * @param transform - A function to transform the original result data.
 * @returns A new query result object with the transformed data if successful, or with `data` as `undefined` otherwise.
 */
export function mapReduxResult<
	Result,
	NewResult,
	T extends CustomResult<QueryDefinition<any, any, any, Result>>
>(
	queryResult: T,
	transform: (data: Result) => NewResult
): CustomResult<QueryDefinition<any, any, any, NewResult>> {
	if (queryResult.isSuccess) {
		return {
			...queryResult,
			data: transform(queryResult.data)
		};
	}

	return {
		...queryResult,
		data: undefined
	};
}

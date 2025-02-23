import {
	QueryStatus,
	type EndpointDefinition,
	type MutationDefinition,
	type QueryDefinition
} from '@reduxjs/toolkit/query';
import type { CustomQuery, CustomResult } from './butlerModule';

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
export function combineQueries<T extends [...CustomResult<any>[]]>(
	...results: T
): CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>> {
	const data = results.every((r) => r.status === QueryStatus.fulfilled)
		? results.map((r) => r.data)
		: undefined;
	const error = results.find((r) => r.error);
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

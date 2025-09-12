import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	QueryStatus,
	type EndpointDefinition,
	type MutationDefinition,
	type QueryDefinition
} from '@reduxjs/toolkit/query';
import type { CustomQuery, CustomResult } from '$lib/state/butlerModule';

export type Result<A> = {
	data?: A;
	status: QueryStatus;
	error?: unknown;
};

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
): CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>> | undefined {
	if (results.length === 0) {
		return;
	}
	const results2 = results.filter(isDefined);

	const data = results2.every((r) => r.data !== undefined) ? results.map((r) => r.data) : undefined;
	const error = results2.find((r) => r.error)?.error;
	const status = (results2.find((r) => r.status === QueryStatus.rejected) ||
		results2.find((r) => r.status === QueryStatus.uninitialized) ||
		results2.find((r) => r.status === QueryStatus.pending) ||
		results2.find((r) => r.status === QueryStatus.fulfilled))!.status;
	return {
		status,
		error,
		data
	} as CustomResult<CustomQuery<{ [K in keyof T]: Exclude<T[K]['data'], undefined> }>>;
}

/**
 * Map the data of a CustomResult, preserving other status fields as they are.
 */
export function mapResult<T, A>(
	result: CustomResult<CustomQuery<T>>,
	fn: (data: T) => A
): CustomResult<CustomQuery<A>> {
	if (result.data === undefined) {
		return result as CustomResult<CustomQuery<A>>;
	}

	return { ...result, data: fn(result.data) } as CustomResult<CustomQuery<A>>;
}

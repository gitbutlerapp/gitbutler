import type {
	EndpointDefinition,
	MutationDefinition,
	QueryDefinition
} from '@reduxjs/toolkit/query';

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

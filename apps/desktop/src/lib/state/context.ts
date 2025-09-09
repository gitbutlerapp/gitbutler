import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';
import type { RootState } from '@reduxjs/toolkit/query';

/**
 *	The api is necessary to create the store, so we need to provide
 *	a way for them to access state and dispatch. In react it's possible
 *	to use the application context since it is available to events
 *	fired by components, while Svelte requires `getContext` only be
 *	used during component initialization.
 */
export type HookContext = {
	/** Without the nested function we get looping reactivity.  */
	getState: () => RootState<any, any, any>;
	getDispatch: () => ThunkDispatch<any, any, UnknownAction>;
	posthog?: PostHogWrapper;
};

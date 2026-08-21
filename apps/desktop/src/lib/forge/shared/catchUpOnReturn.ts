import type { SubscriptionOptions } from "@reduxjs/toolkit/query";

/**
 * For forge state that changes while the app isn't looking - checks, the PR, its
 * merge status, the review listing. Their polls widen to minutes, so focus
 * catches up sooner. Not for data that only changes when we change it.
 */
export const catchUpOnReturn = {
	refetchOnFocus: true,
	refetchOnReconnect: true,
} as const satisfies SubscriptionOptions;

import { getContext, setContext } from 'svelte';
import type { WritableReactive } from '@gitbutler/shared/storeUtils';

type BreadcrumbsContext = {
	ownerSlug?: string;
	projectSlug?: string;
	branchId?: string;
	changeId?: string;
};

const breadcrumbsContextKey = Symbol('breadcrumbs-context');

export function getBreadcrumbsContext(): WritableReactive<BreadcrumbsContext> {
	const context = getContext<WritableReactive<BreadcrumbsContext> | undefined>(
		breadcrumbsContextKey
	);
	if (!context) {
		throw new Error('Breadcrumb context is empty');
	}

	return context;
}

export function initializeBreadcrumbsContext() {
	const context = $state<WritableReactive<BreadcrumbsContext>>({ current: {} });

	setContext(breadcrumbsContextKey, context);
}

export function cleanBreadcrumbs() {
	const context = getBreadcrumbsContext();
	context.current = {};
}

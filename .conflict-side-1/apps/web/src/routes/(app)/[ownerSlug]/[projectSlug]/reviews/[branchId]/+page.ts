import type { PageLoad } from './$types';
import type { ProjectReviewParameters } from '@gitbutler/shared/routing/webRoutes.svelte';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectReviewParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug,
		branchId: params.branchId
	};
};

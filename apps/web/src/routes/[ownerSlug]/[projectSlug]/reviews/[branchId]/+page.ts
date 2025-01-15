import type { ProjectReviewParameters } from '$lib/routing';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectReviewParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug,
		branchId: params.branchId
	};
};

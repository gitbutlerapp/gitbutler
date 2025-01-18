import type { PageLoad } from './$types';
import type { ProjectReviewCommitParameters } from '@gitbutler/shared/routing/webRoutes';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectReviewCommitParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug,
		branchId: params.branchId,
		changeId: params.changeId
	};
};

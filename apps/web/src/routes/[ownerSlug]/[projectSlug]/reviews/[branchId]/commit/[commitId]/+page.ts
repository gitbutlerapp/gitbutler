import type { PageLoad } from './$types';
import type { ProjectReviewCommitParameters } from '../../../../../types';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectReviewCommitParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug,
		branchId: params.branchId,
		commitId: params.commitId
	};
};

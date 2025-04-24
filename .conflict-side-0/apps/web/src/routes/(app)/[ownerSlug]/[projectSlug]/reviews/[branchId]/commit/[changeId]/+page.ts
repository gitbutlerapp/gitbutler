import type { PageLoad } from './$types';
import type { ProjectReviewCommitParameters } from '@gitbutler/shared/routing/webRoutes.svelte';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectReviewCommitParameters> = async ({ params, url }) => {
	const messageUuid = url.searchParams.get('m') ?? undefined;
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug,
		branchId: params.branchId,
		changeId: params.changeId,
		messageUuid
	};
};

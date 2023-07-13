import type { PageLoad } from './$types';
import { git } from '$lib/api';

export const load: PageLoad = async ({ params }) => ({
	activity: git.activities.Activities({ projectId: params.projectId })
});

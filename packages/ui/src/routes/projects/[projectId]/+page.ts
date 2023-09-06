import type { PageLoad } from './$types';
import { getActivitiesStore } from '$lib/api/git/activities';

export const load: PageLoad = async ({ params }) => ({
	activity: getActivitiesStore({ projectId: params.projectId })
});

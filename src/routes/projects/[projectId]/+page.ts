import type { PageLoad } from './$types';
import { git } from '$lib/api';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ params }) => ({
	activity: git.activities.Activities({ projectId: params.projectId })
}));

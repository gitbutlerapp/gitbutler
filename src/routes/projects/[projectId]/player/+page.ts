import { error, redirect } from '@sveltejs/kit';
import { format, compareDesc } from 'date-fns';
import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ parent, url, params }) => {
	const { sessions } = await parent();
	const latestDate = await sessions.load().then((sessions) =>
		sessions
			.map((session) => session.meta.startTimestampMs)
			.sort(compareDesc)
			.shift()
	);
	if (!latestDate) throw error(404, 'No sessions found');
	throw redirect(
		302,
		`/projects/${params.projectId}/player/${format(latestDate, 'yyyy-MM-dd')}/${url.search}`
	);
});

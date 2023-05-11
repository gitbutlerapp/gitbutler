import { redirect, error } from '@sveltejs/kit';
import { format } from 'date-fns';
import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ parent, params, url }) => {
	const { sessions, projectId } = await parent();
	const dateSessions = await sessions
		.load()
		.then((sessions) =>
			sessions.filter(
				(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date
			)
		);
	if (!dateSessions.length) throw error(404, 'No sessions found');
	const firstSession = dateSessions[dateSessions.length - 1];
	throw redirect(
		302,
		`/projects/${projectId}/player/${params.date}/${firstSession.id}${url.search}`
	);
});

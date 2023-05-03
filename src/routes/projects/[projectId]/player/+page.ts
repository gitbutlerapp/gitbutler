import { redirect } from '@sveltejs/kit';
import { format } from 'date-fns';
import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ parent, url }) => {
	const { sessions, projectId } = await parent();
	const date = format(new Date(), 'yyyy-MM-dd');
	const dateSessions = await sessions
		.load()
		.then((sessions) =>
			sessions.filter((session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === date)
		);
	const firstSession = dateSessions[dateSessions.length - 1];
	throw redirect(302, `/projects/${projectId}/player/${date}/${firstSession.id}${url.search}`);
});

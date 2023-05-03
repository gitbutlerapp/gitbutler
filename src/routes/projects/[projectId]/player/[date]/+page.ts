import { redirect } from '@sveltejs/kit';
import { format } from 'date-fns';
import { get } from '@square/svelte-store';
import type { PageLoad } from './$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ parent, params, url }) => {
	const { sessions, projectId } = await parent();
	await sessions.load();
	const dateSessions = get(sessions).filter(
		(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date
	);
	const firstSession = dateSessions[dateSessions.length - 1];
	throw redirect(
		302,
		`/projects/${projectId}/player/${params.date}/${firstSession.id}${url.search}`
	);
});

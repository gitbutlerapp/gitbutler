import { redirect, error } from '@sveltejs/kit';
import { format } from 'date-fns';
import type { PageLoad } from './$types';
import { getSessionStore } from '$lib/stores/sessions';

export const load: PageLoad = async ({ params, url }) => {
	const sessions = getSessionStore(params.projectId);
	const dateSessions = (await sessions.load()).filter(
		(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date
	);
	if (!dateSessions.length) throw error(404, 'No sessions found');
	const firstSession = dateSessions[dateSessions.length - 1];
	throw redirect(
		302,
		`/projects/${params.projectId}/player/${params.date}/${firstSession.id}${url.search}`
	);
};

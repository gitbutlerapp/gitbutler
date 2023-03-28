import { redirect } from '@sveltejs/kit';
import { format } from 'date-fns';
import { get } from 'svelte/store';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ parent, params, url }) => {
	const { sessions, projectId } = await parent();
	const dateSessions = get(sessions).filter(
		(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date
	);
	const firstSession = dateSessions[dateSessions.length - 1];
	throw redirect(
		302,
		`/projects/${projectId}/player/${params.date}/${firstSession.id}${url.search}`
	);
};

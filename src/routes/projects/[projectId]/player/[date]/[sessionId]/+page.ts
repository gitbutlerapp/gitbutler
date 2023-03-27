import type { Delta } from '$lib/deltas';
import type { Session } from '$lib/sessions';
import { asyncDerived } from '@square/svelte-store';
import { format } from 'date-fns';
import type { PageLoad } from './$types';

const enrichSession = async (projectId: string, session: Session) => {
	const sessionsModule = await import('$lib/sessions');
	const deltasModule = await import('$lib/deltas');
	const files = await sessionsModule.listFiles({ projectId, sessionId: session.id });
	const deltas = await deltasModule.list({ projectId, sessionId: session.id }).then((deltas) =>
		Object.entries(deltas)
			.flatMap(([path, deltas]) => deltas.map((delta) => [path, delta] as [string, Delta]))
			.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
	);
	const deltasFiles = new Set(deltas.map(([path]) => path));
	return {
		...session,
		files: Object.fromEntries(
			Object.entries(files).filter(([filepath]) => deltasFiles.has(filepath))
		),
		deltas
	};
};

export const load: PageLoad = async ({ params, parent, url }) => {
	const { sessions } = await parent();
	return {
		sessions: asyncDerived(sessions, async (sessions) =>
			Promise.all(
				sessions
					.filter((session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date)
					.map((session) => enrichSession(params.projectId, session))
			).then((sessions) => {
				sessions = sessions.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
				const fileFilter = url.searchParams.get('file');
				if (fileFilter) {
					sessions = sessions
						.filter((session) => session.files[fileFilter])
						.map((session) => ({
							...session,
							deltas: session.deltas.filter(([path]) => path === fileFilter),
							files: {
								[fileFilter]: session.files[fileFilter]
							}
						}));
				}
				return sessions;
			})
		)
	};
};

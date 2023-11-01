import { asyncDerived, derived, writable } from '@square/svelte-store';
import type { LayoutLoad } from './$types';
import { format } from 'date-fns';
import { page } from '$app/stores';
import { listDeltas, type Delta } from '$lib/api/ipc/deltas';
import { list } from '$lib/api/ipc/files';

export const load: LayoutLoad = async ({ parent, params, url }) => {
	const { sessions } = await parent();
	const dateSessions = asyncDerived([sessions], async ([sessions]) => {
		return sessions.filter(
			(session) => format(session.meta.startTimestampMs, 'yyyy-MM-dd') === params.date
		);
	});
	const filter = writable(url.searchParams.get('file'));
	const projectId = writable(params.projectId);

	const loadedDeltas = asyncDerived(dateSessions, async (sessions) => {
		return Promise.all(
			sessions.map(async (s) => {
				return {
					sessionId: s.id,
					deltas: await listDeltas({ projectId: params.projectId, sessionId: s.id })
				};
			})
		);
	});

	const loadedDateDeltas = asyncDerived(loadedDeltas, async (deltas) => {
		const deltas2 = deltas.map((s) => {
			return {
				sessionId: s.sessionId,
				deltas: Object.entries(s.deltas)
					.flatMap(([path, deltas]) =>
						(deltas || []).map((delta) => [path, delta] as [string, Delta])
					)
					.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
			};
		});
		const deltasMap: { [k: string]: [string, Delta][] } = {};
		for (let i = 0; i < deltas2.length; i++) {
			const deltas = deltas2[i].deltas;
			if (deltas && Object.keys(deltas)) {
				deltasMap[deltas2[i].sessionId] = deltas2[i].deltas;
			}
		}
		return deltasMap;
	});

	const loadedDateDeltas2 = asyncDerived(loadedDeltas, async (deltas) => {
		const deltasMap: { [k: string]: Partial<Record<string, Delta[]>> } = {};
		for (let i = 0; i < deltas.length; i++) {
			if (deltas && Object.keys(deltas)) {
				deltasMap[deltas[i].sessionId] = deltas[i].deltas;
			}
		}
		return deltasMap;
	});

	const loadedFiles = asyncDerived(loadedDateDeltas2, async (deltas) => {
		const sessionIds = Object.keys(deltas);
		const files = sessionIds.map(async (sessionId) => {
			const filenames = Object.keys(deltas[sessionId] || {});
			const p = { projectId: params.projectId, sessionId: sessionId, paths: filenames };
			return {
				sessionId: sessionId,
				files: Object.fromEntries(
					Object.entries(await list(p)).map(([path, file]) => {
						if (file?.type === 'utf8') {
							return [path, file.value];
						} else {
							return [path, undefined];
						}
					})
				)
			};
		});
		const resolvedFiles = await Promise.all(files);
		const filesMap: { [y: string]: { [k: string]: string | undefined } } = {};
		for (let i = 0; i < resolvedFiles.length; i++) {
			const files = resolvedFiles[i].files;
			if (files && Object.keys(files)) {
				filesMap[resolvedFiles[i].sessionId] = files;
			}
		}
		return filesMap;
	});

	const richSessions = asyncDerived(
		[dateSessions, loadedDateDeltas, loadedFiles, projectId, filter],
		async ([sessions, loadedDateDeltas, loadedFiles]) => {
			return sessions.map((session) => ({
				...session,
				deltas: loadedDateDeltas[session.id],
				files: loadedFiles[session.id]
			}));
		}
	);

	const richSessions2 = asyncDerived(
		[dateSessions, loadedDateDeltas2, loadedFiles],
		async ([sessions, loadedDateDeltas, loadedFiles]) =>
			sessions.map((session) => ({
				...session,
				deltas: loadedDateDeltas[session.id],
				files: loadedFiles[session.id]
			}))
	);

	const currentSessionId = writable('');

	const currentSession = derived(
		[page, richSessions, currentSessionId],
		([page, richSessions, currentSessionId]) => {
			const val =
				richSessions?.find((s) => s.id === currentSessionId) ??
				richSessions?.find((s) => s.id === page.params.sessionId);
			return val;
		}
	);
	return {
		currentFilepath: writable(''),
		currentTimestamp: writable(-1),
		currentSessionId,
		dateSessions,
		richSessions,
		richSessions2,
		filter,
		projectId,
		currentSession,
		loadedDateDeltas,
		loadedDeltas,
		loadedFiles,
		sessions
	};
};

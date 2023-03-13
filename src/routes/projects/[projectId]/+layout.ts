import type { LayoutLoad } from './$types';
import { building } from '$app/environment';
import { readable, derived } from 'svelte/store';
import type { Session } from '$lib/sessions';
import type { UISession } from '$lib/uisessions';
import type { Status } from '$lib/statuses';
import { asyncDerived } from '@square/svelte-store';
import type { Delta } from '$lib/deltas';
import { startOfDay } from 'date-fns';
import type { Activity } from '$lib/sessions';

export const prerender = false;
export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();

	const filesStatus = building
		? readable<Status[]>([])
		: await (await import('$lib/statuses')).default({ projectId: params.projectId });

	const sessions = building
		? readable<Session[]>([])
		: await (await import('$lib/sessions')).default({ projectId: params.projectId });
	const orderedSessions = derived(sessions, (sessions) => {
		return sessions.slice().sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
	});
	const recentActivity = derived(sessions, (sessions) => {
		const recentActivity: Activity[] = [];
		sessions.forEach((session) => {
			session.activity.forEach((activity) => {
				recentActivity.push(activity);
			});
		});
		const activitySorted = recentActivity.sort((a, b) => {
			return b.timestampMs - a.timestampMs;
		});
		return activitySorted.slice(0, 20);
	});

	let latestDeltasByDateByFile = readable<Record<number, Record<string, Delta[][]>[]>>({});
	if (!building) {
		latestDeltasByDateByFile = asyncDerived(sessions, async (sessions) => {
			const dateSessions: Record<number, Session[]> = {};
			sessions.forEach((session) => {
				const date = startOfDay(new Date(session.meta.startTimestampMs));
				if (dateSessions[date.getTime()]) {
					dateSessions[date.getTime()]?.push(session);
				} else {
					dateSessions[date.getTime()] = [session];
				}
			});

			const latestDateSessions: Record<number, Session[]> = Object.fromEntries(
				Object.entries(dateSessions)
					.sort((a, b) => parseInt(b[0]) - parseInt(a[0]))
					.slice(0, 3)
			); // Only show the last 3 days

			const listDeltas = (await import('$lib/deltas')).list;

			return Object.fromEntries(
				await Promise.all(
					Object.keys(latestDateSessions).map(async (date: string) => {
						const sessionsByFile = await Promise.all(
							latestDateSessions[parseInt(date)].map(async (session) => {
								const sessionDeltas = await listDeltas({
									projectId: params.projectId ?? '',
									sessionId: session.id
								});

								const fileDeltas: Record<string, Delta[][]> = {};

								Object.keys(sessionDeltas).forEach((filePath) => {
									if (sessionDeltas[filePath].length > 0) {
										if (fileDeltas[filePath]) {
											fileDeltas[filePath]?.push(sessionDeltas[filePath]);
										} else {
											fileDeltas[filePath] = [sessionDeltas[filePath]];
										}
									}
								});
								return fileDeltas;
							})
						);
						return [date, sessionsByFile];
					})
				)
			);
		});
	}

	return {
		project: projects.get(params.projectId),
		projectId: params.projectId,
		sessions: orderedSessions,
		filesStatus: filesStatus,
		recentActivity: recentActivity,
		latestDeltasByDateByFile: latestDeltasByDateByFile
	};
};

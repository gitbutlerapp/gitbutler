import type { LayoutLoad } from './$types';
import { building } from '$app/environment';
import { readable, derived } from 'svelte/store';
import type { Session } from '$lib/sessions';
import type { Status } from '$lib/statuses';
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

	return {
		project: projects.get(params.projectId),
		projectId: params.projectId,
		sessions: orderedSessions,
		filesStatus: filesStatus,
		recentActivity: recentActivity,
	};
};

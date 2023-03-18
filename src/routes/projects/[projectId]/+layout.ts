import type { LayoutLoad } from './$types';
import { building } from '$app/environment';
import { readable, derived } from 'svelte/store';
import type { Session } from '$lib/sessions';
import type { Status } from '$lib/statuses';
import type { Activity } from '$lib/sessions';
import { subDays, getTime } from 'date-fns';

export const prerender = false;
export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();

	const filesStatus = building
		? readable<Status[]>([])
		: await (await import('$lib/statuses')).default({ projectId: params.projectId });

	const sessionsFromLastFourDays = building
		? readable<Session[]>([])
		: await (
				await import('$lib/sessions')
		  ).default({
				projectId: params.projectId,
				earliestTimestampMs: getTime(subDays(new Date(), 4))
		  });
	const orderedSessionsFromLastFourDays = derived(sessionsFromLastFourDays, (sessions) => {
		return sessions.slice().sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
	});
	const recentActivity = derived(sessionsFromLastFourDays, (sessions) => {
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
	const user = building
  ? {
      ...readable<undefined>(undefined),
      set: () => {
        throw new Error('not implemented');
      },
      delete: () => {
        throw new Error('not implemented');
      }
    }
  : await (await import('$lib/users')).default();

	return {
		user: user,
		project: projects.get(params.projectId),
		projectId: params.projectId,
		orderedSessionsFromLastFourDays: orderedSessionsFromLastFourDays,
		filesStatus: filesStatus,
		recentActivity: recentActivity
	};
};

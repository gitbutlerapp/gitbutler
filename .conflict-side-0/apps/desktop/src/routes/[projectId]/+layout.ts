import { HistoryService } from '$lib/history/history';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params }) => {
	const historyService = new HistoryService(params.projectId);
	return {
		historyService,
		projectId: params.projectId
	};
};

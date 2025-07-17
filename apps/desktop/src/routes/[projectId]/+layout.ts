import { GitBranchService } from '$lib/branches/gitBranch';
import { FetchSignal } from '$lib/fetchSignal/fetchSignal.js';
// import { UncommitedFilesWatcher } from '$lib/files/watcher';
import { TemplateService } from '$lib/forge/templateService';
import { HistoryService } from '$lib/history/history';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	const { projectMetrics } = await parent();

	const projectId = params.projectId;

	const fetchSignal = new FetchSignal(projectId);

	const historyService = new HistoryService(projectId);
	const templateService = new TemplateService(projectId);

	const gitBranchService = new GitBranchService(projectId);

	return {
		templateService,
		historyService,
		projectId,
		gitBranchService,
		projectMetrics,
		fetchSignal
		// uncommitedFileWatcher,
		// Cloud-related services
		// syncedSnapshotService,
	};
};

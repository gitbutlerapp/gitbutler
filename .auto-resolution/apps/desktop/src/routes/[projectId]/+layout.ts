import { getUserErrorCode } from '$lib/backend/ipc';
import { GitBranchService } from '$lib/branches/gitBranch';
import { FetchSignal } from '$lib/fetchSignal/fetchSignal.js';
import { UncommitedFilesWatcher } from '$lib/files/watcher';
import { TemplateService } from '$lib/forge/templateService';
import { HistoryService } from '$lib/history/history';
import { StackPublishingService } from '$lib/history/stackPublishingService';
import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
import { ModeService } from '$lib/mode/modeService';
import { type Project } from '$lib/project/project';
import { ProjectService } from '$lib/project/projectService';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	const { projectsService, commandService, userService, projectMetrics } = await parent();

	const projectId = params.projectId;
	projectsService.setLastOpenedProject(projectId);

	// Getting the project should be one of few, if not the only await expression in
	// this function. It delays drawing the page, but currently the benefit from having this
	// synchronously available are much greater than the cost.
	// However, what's awaited here is required for proper error handling,
	// and by now this is fast enough to not be an impediment.
	let project: Project | undefined = undefined;
	try {
		project = await projectsService.getProject(projectId);
	} catch (err: any) {
		const errorCode = getUserErrorCode(err);
		throw error(400, {
			errorCode,
			message: err.message
		});
	}

	const projectService = new ProjectService(projectsService, projectId);

	const modeService = new ModeService(projectId);
	const fetchSignal = new FetchSignal(projectId);

	const historyService = new HistoryService(projectId);
	const templateService = new TemplateService(projectId);

	const gitBranchService = new GitBranchService(projectId);

	const uncommitedFileWatcher = new UncommitedFilesWatcher(project);
	const syncedSnapshotService = new SyncedSnapshotService(
		commandService,
		userService.user,
		projectsService.getProjectStore(projectId)
	);
	const stackPublishingService = new StackPublishingService(
		commandService,
		userService.user,
		projectsService.getProjectStore(projectId)
	);

	return {
		templateService,
		historyService,
		projectId,
		project,
		projectService,
		gitBranchService,
		projectMetrics,
		modeService,
		fetchSignal,
		uncommitedFileWatcher,
		// Cloud-related services
		syncedSnapshotService,
		stackPublishingService
	};
};

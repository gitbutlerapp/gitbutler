import { getUserErrorCode, invoke } from '$lib/backend/ipc';
import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
import { BranchController } from '$lib/branches/branchController';
import { BranchListingService } from '$lib/branches/branchListing';
import { BranchDragActionsFactory } from '$lib/branches/dragActions.js';
import { GitBranchService } from '$lib/branches/gitBranch';
import { VirtualBranchService } from '$lib/branches/virtualBranchService';
import { CommitDragActionsFactory } from '$lib/commits/dragActions.js';
import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
import { FetchSignal } from '$lib/fetchSignal/fetchSignal.js';
import { UncommitedFilesWatcher } from '$lib/files/watcher';
import { TemplateService } from '$lib/forge/templateService';
import { HistoryService } from '$lib/history/history';
import { StackPublishingService } from '$lib/history/stackPublishingService';
import { SyncedSnapshotService } from '$lib/history/syncedSnapshotService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { ModeService } from '$lib/mode/modeService';
import { type Project } from '$lib/project/project';
import { ProjectService } from '$lib/project/projectService';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	const { projectsService, commandService, userService, posthog } = await parent();

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
		await invoke('set_project_active', { id: projectId });
	} catch (err: any) {
		const errorCode = getUserErrorCode(err);
		throw error(400, {
			errorCode,
			message: err.message
		});
	}

	const projectService = new ProjectService(projectsService, projectId);

	const projectMetrics = new ProjectMetrics(projectId);

	const modeService = new ModeService(projectId);
	const fetchSignal = new FetchSignal(projectId);

	const historyService = new HistoryService(projectId);
	const baseBranchService = new BaseBranchService(projectId);
	const templateService = new TemplateService(projectId);

	const branchListingService = new BranchListingService(projectId);
	const gitBranchService = new GitBranchService(projectId);

	const vbranchService = new VirtualBranchService(
		projectId,
		projectMetrics,
		branchListingService,
		modeService
	);

	const branchController = new BranchController(
		projectId,
		vbranchService,
		baseBranchService,
		branchListingService,
		posthog
	);

	const branchDragActionsFactory = new BranchDragActionsFactory(branchController);
	const commitDragActionsFactory = new CommitDragActionsFactory(branchController, project);
	const stackingReorderDropzoneManagerFactory = new StackingReorderDropzoneManagerFactory(
		branchController
	);

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
		baseBranchService,
		templateService,
		branchController,
		historyService,
		projectId,
		project,
		projectService,
		gitBranchService,
		vbranchService,
		projectMetrics,
		modeService,
		fetchSignal,

		// These observables are provided for convenience
		branchDragActionsFactory,
		commitDragActionsFactory,
		stackingReorderDropzoneManagerFactory,
		branchListingService,
		uncommitedFileWatcher,

		// Cloud-related services
		syncedSnapshotService,
		stackPublishingService
	};
};

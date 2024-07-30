import { invoke } from '$lib/backend/ipc';
import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
import { BranchDragActionsFactory } from '$lib/branches/dragActions.js';
import { CommitDragActionsFactory } from '$lib/commits/dragActions.js';
import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
import { FetchSignal } from '$lib/fetchSignal/fetchSignal.js';
import { HeadService } from '$lib/head/headService';
import { HistoryService } from '$lib/history/history';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { RemoteBranchService } from '$lib/stores/remoteBranches';
import { BranchController } from '$lib/vbranches/branchController';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { error } from '@sveltejs/kit';
import type { Project } from '$lib/backend/projects';
import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params, parent }) => {
	// prettier-ignore
	const {
        authService,
        projectService,
    } = await parent();

	const projectId = params.projectId;
	// Getting the project should be one of few, if not the only await expression in
	// this function. It delays drawing the page, but currently the benefit from having this
	// synchronously available are much greater than the cost.
	// However, what's awaited here is required for proper error handling,
	// and by now this is fast enough to not be an impediment.
	let project: Project | undefined = undefined;
	try {
		project = await projectService.getProject(projectId);
		await invoke('set_project_active', { id: projectId });
	} catch (err: any) {
		throw error(400, {
			message: err.message
		});
	}

	const projectMetrics = new ProjectMetrics(projectId);

	const headService = new HeadService(projectId);
	const fetchSignal = new FetchSignal(projectId);

	const historyService = new HistoryService(projectId);
	const baseBranchService = new BaseBranchService(projectId);
	const remoteBranchService = new RemoteBranchService(projectId, projectMetrics);

	const vbranchService = new VirtualBranchService(projectId, projectMetrics, remoteBranchService);

	const branchController = new BranchController(
		projectId,
		vbranchService,
		remoteBranchService,
		baseBranchService
	);

	const branchDragActionsFactory = new BranchDragActionsFactory(branchController);
	const commitDragActionsFactory = new CommitDragActionsFactory(branchController, project);
	const reorderDropzoneManagerFactory = new ReorderDropzoneManagerFactory(branchController);

	return {
		authService,
		baseBranchService,
		branchController,
		historyService,
		projectId,
		project,
		remoteBranchService,
		vbranchService,
		projectMetrics,
		headService,
		fetchSignal,

		// These observables are provided for convenience
		branchDragActionsFactory,
		commitDragActionsFactory,
		reorderDropzoneManagerFactory
	};
};

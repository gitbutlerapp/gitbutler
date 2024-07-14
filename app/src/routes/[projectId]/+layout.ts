import { invoke } from '$lib/backend/ipc';
import { BranchDragActionsFactory } from '$lib/branches/dragActions.js';
import { CommitDragActionsFactory } from '$lib/commits/dragActions.js';
import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
import { HistoryService } from '$lib/history/history';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { getFetchNotifications } from '$lib/stores/fetches';
import { getHeads } from '$lib/stores/head';
import { RemoteBranchService } from '$lib/stores/remoteBranches';
import { BaseBranchService } from '$lib/vbranches/baseBranch';
import { BranchController } from '$lib/vbranches/branchController';
import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
import { error } from '@sveltejs/kit';
import { map } from 'rxjs';
import type { Project } from '$lib/backend/projects';

export const prerender = false;

export async function load({ params, parent }) {
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

	const fetches$ = getFetchNotifications(projectId);
	const heads$ = getHeads(projectId);
	const gbBranchActive$ = heads$.pipe(map((head) => head === 'gitbutler/integration'));

	const historyService = new HistoryService(projectId);
	const baseBranchService = new BaseBranchService(projectId, fetches$, heads$);
	const vbranchService = new VirtualBranchService(projectId, projectMetrics, gbBranchActive$);

	const remoteBranchService = new RemoteBranchService(
		projectId,
		projectMetrics,
		fetches$,
		heads$,
		baseBranchService.base$
	);
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

		// These observables are provided for convenience
		gbBranchActive$,
		branchDragActionsFactory,
		commitDragActionsFactory,
		reorderDropzoneManagerFactory
	};
}

import { invoke } from '$lib/backend/ipc';
import { BranchDragActionsFactory } from '$lib/branches/dragActions.js';
import { BranchService } from '$lib/branches/service';
import { CommitDragActionsFactory } from '$lib/commits/dragActions.js';
import { ReorderDropzoneManagerFactory } from '$lib/dragging/reorderDropzoneManager';
import { HistoryService } from '$lib/history/history';
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
        githubService,
        projectService,
        remoteUrl$,
    } = await parent();

	const projectId = params.projectId;
	// Getting the project should be one of few, if not the only await expression in
	// this function. It delays drawing the page, but currently the benefit from having this
	// synchronously available are much greater than the cost.
	let project: Project | undefined = undefined;
	try {
		project = await projectService.getProject(projectId);
		invoke('set_project_active', { id: projectId }).then((_r) => {});
	} catch (err: any) {
		throw error(400, {
			message: err.message
		});
	}

	const fetches$ = getFetchNotifications(projectId);
	const heads$ = getHeads(projectId);
	const gbBranchActive$ = heads$.pipe(map((head) => head === 'gitbutler/integration'));

	const historyService = new HistoryService(projectId);
	const baseBranchService = new BaseBranchService(projectId, remoteUrl$, fetches$, heads$);
	const vbranchService = new VirtualBranchService(projectId, gbBranchActive$);

	const remoteBranchService = new RemoteBranchService(
		projectId,
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
	const branchService = new BranchService(
		vbranchService,
		remoteBranchService,
		githubService,
		branchController
	);

	const branchDragActionsFactory = new BranchDragActionsFactory(branchController);
	const commitDragActionsFactory = new CommitDragActionsFactory(branchController, project);
	const reorderDropzoneManagerFactory = new ReorderDropzoneManagerFactory(branchController);

	return {
		authService,
		baseBranchService,
		branchController,
		branchService,
		githubService,
		historyService,
		projectId,
		project,
		remoteBranchService,
		vbranchService,

		// These observables are provided for convenience
		gbBranchActive$,
		branchDragActionsFactory,
		commitDragActionsFactory,
		reorderDropzoneManagerFactory
	};
}

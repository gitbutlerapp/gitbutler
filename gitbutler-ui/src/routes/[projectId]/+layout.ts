import { BranchService } from '$lib/branches/service';
import { getFetchNotifications } from '$lib/stores/fetches';
import { getHeads } from '$lib/stores/head';
import { RemoteBranchService } from '$lib/stores/remoteBranches';
import { BranchController } from '$lib/vbranches/branchController';
import { BaseBranchService, VirtualBranchService } from '$lib/vbranches/branchStoresCache';
import { map } from 'rxjs';

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
	const project$ = projectService.getProject(projectId);
	const fetches$ = getFetchNotifications(projectId);
	const heads$ = getHeads(projectId);
	const gbBranchActive$ = heads$.pipe(map((head) => head == 'gitbutler/integration'));

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

	return {
		authService,
		baseBranchService,
		branchController,
		branchService,
		githubService,
		projectId,
		remoteBranchService,
		vbranchService,

		// These observables are provided for convenience
		project$,
		gbBranchActive$
	};
}

import { DeltasService } from '$lib/stores/deltas';
import { getFetchNotifications } from '$lib/stores/fetches';
import { getHeads } from '$lib/stores/head';
import { getSessions } from '$lib/stores/sessions';
import { BranchController } from '$lib/vbranches/branchController';
import { getGithubContext } from '$lib/stores/github';
import { BaseBranchService, VirtualBranchService } from '$lib/vbranches/branchStoresCache';
import type { LayoutLoad } from './$types';
import { PrService } from '$lib/github/pullrequest';
import { of, shareReplay, switchMap } from 'rxjs';
import { RemoteBranchService } from '$lib/stores/remoteBranches';

export const prerender = false;

export const load: LayoutLoad = async ({ params, parent }) => {
	const { user$ } = await parent();
	const projectId = params.projectId;
	const fetches$ = getFetchNotifications(projectId);
	const heads$ = getHeads(projectId);
	const sessions$ = getSessions(projectId);
	const baseBranchService = new BaseBranchService(projectId, fetches$, heads$);
	const remoteBranchService = new RemoteBranchService(
		projectId,
		fetches$,
		heads$,
		baseBranchService.base$
	);
	const sessionId$ = sessions$.pipe(
		switchMap((sessions) => of(sessions[0].id)),
		shareReplay(1)
	);
	const deltaService = new DeltasService(projectId, sessionId$);

	const githubContext$ = getGithubContext(user$, baseBranchService.base$);

	const vbranchService = new VirtualBranchService(
		projectId,
		deltaService.deltas$,
		sessionId$,
		heads$,
		baseBranchService.base$
	);

	const prService = new PrService(githubContext$);

	const branchController = new BranchController(
		projectId,
		vbranchService,
		remoteBranchService,
		baseBranchService,
		sessions$
	);

	return {
		projectId,
		branchController,
		baseBranchService,
		prService,
		vbranchService,
		githubContext$,
		remoteBranchService,
		user$
	};
};

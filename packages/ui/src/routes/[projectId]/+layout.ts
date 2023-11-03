import { getProjectStore } from '$lib/backend/projects';
import { getDeltasStore } from '$lib/stores/deltas';
import { getFetchesStore } from '$lib/stores/fetches';
import { getHeadsStore } from '$lib/stores/head';
import { getSessionStore } from '$lib/stores/sessions';
import { BranchController } from '$lib/vbranches/branchController';
import { getGitHubContextStore } from '$lib/stores/github';
import {
	getBaseBranchStore,
	getRemoteBranchStore,
	getVirtualBranchStore,
	getWithContentStore
} from '$lib/vbranches/branchStoresCache';
import type { LayoutLoad } from './$types';
import { userStore } from '$lib/stores/user';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	const projectId = params.projectId;
	const fetchStore = getFetchesStore(projectId);
	const deltasStore = getDeltasStore(projectId, undefined, true);
	const headStore = getHeadsStore(projectId);
	const sessionsStore = getSessionStore(projectId);
	const baseBranchStore = getBaseBranchStore(projectId, fetchStore, headStore);
	const remoteBranchStore = getRemoteBranchStore(projectId, fetchStore, headStore, baseBranchStore);
	const vbranchStore = getVirtualBranchStore(
		projectId,
		deltasStore,
		sessionsStore,
		headStore,
		baseBranchStore
	);
	const branchesWithContent = getWithContentStore(projectId, sessionsStore, vbranchStore);

	const vbranchesState = vbranchStore.state;
	const branchesState = branchesWithContent.state;
	const baseBranchesState = baseBranchStore.state;

	const branchController = new BranchController(
		projectId,
		vbranchStore,
		remoteBranchStore,
		baseBranchStore,
		sessionsStore
	);

	const githubContextStore = getGitHubContextStore(userStore, baseBranchStore);
	const project = getProjectStore({ id: params.projectId });

	return {
		projectId,
		vbranchStore,
		branchesWithContent,
		branchController,
		vbranchesState,
		branchesState,
		baseBranchesState,
		sessionsStore,
		deltasStore,
		baseBranchStore,
		remoteBranchStore,
		project,
		githubContextStore
	};
};

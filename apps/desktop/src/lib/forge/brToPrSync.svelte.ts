import { getPr } from '$lib/forge/getPr.svelte';
import { getForgePrService } from '$lib/forge/interface/forgePrService';
import { updateButRequestPrDescription } from '$lib/forge/shared/prFooter';
import { ProjectService } from '$lib/project/projectService';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
import { getContext, inject } from '@gitbutler/shared/context';
import { isFound, map } from '@gitbutler/shared/network/loadable';
import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { AppState } from '@gitbutler/shared/redux/store.svelte';
import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
import { untrack } from 'svelte';
import { get } from 'svelte/store';
import type { PatchSeries } from '$lib/branches/branch';
import type { DetailedPullRequest } from '$lib/forge/interface/types';
import type { Branch as CloudBranch } from '@gitbutler/shared/branches/types';
import type { Project as CloudProject } from '@gitbutler/shared/organizations/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function syncBrToPr(branch: Reactive<PatchSeries>) {
	const pr = getPr(branch);

	const [projectService, appState, latestBranchLookupService, cloudBranchService] = inject(
		ProjectService,
		AppState,
		LatestBranchLookupService,
		CloudBranchService
	);
	const project = readableToReactive(projectService.project);

	const cloudProject = $derived(
		project.current?.api?.repository_id
			? getProjectByRepositoryId(project.current.api.repository_id)
			: undefined
	);

	const cloudBranchUuid = $derived(
		map(cloudProject?.current, (cloudProject) => {
			if (!branch.current.reviewId) return;

			return lookupLatestBranchUuid(
				appState,
				latestBranchLookupService,
				cloudProject.owner,
				cloudProject.slug,
				branch.current.reviewId
			);
		})
	);

	const cloudBranch = $derived(
		map(cloudBranchUuid?.current, (cloudBranchUuid) => {
			return getBranchReview(appState, cloudBranchService, cloudBranchUuid);
		})
	);

	$effect(() => {
		if (isFound(cloudProject?.current) && isFound(cloudBranch?.current) && pr.current) {
			const cloudProjectValue = cloudProject.current.value;
			const cloudBranchValue = cloudBranch.current.value;
			const prValue = pr.current;

			untrack(() => untrackedUpdate(prValue, cloudProjectValue, cloudBranchValue));
		}
	});
}

function untrackedUpdate(pr: DetailedPullRequest, project: CloudProject, branch: CloudBranch) {
	const prService = get(getForgePrService());
	const webRoutes = getContext(WebRoutesService);
	if (!prService) return;

	const butlerRequestUrl = webRoutes.projectReviewBranchUrl({
		branchId: branch.branchId,
		projectSlug: project.slug,
		ownerSlug: project.owner
	});

	updateButRequestPrDescription(prService, pr.body || '\n', pr.number, butlerRequestUrl, branch);
}

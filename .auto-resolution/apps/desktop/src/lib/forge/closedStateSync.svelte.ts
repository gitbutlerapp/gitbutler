import { ProjectService } from '$lib/project/projectService';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
import { BranchStatus } from '@gitbutler/shared/branches/types';
import { inject } from '@gitbutler/shared/context';
import { isFound, map } from '@gitbutler/shared/network/loadable';
import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { AppState } from '@gitbutler/shared/redux/store.svelte';
import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
import type { PatchSeries } from '$lib/branches/branch';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function closedStateSync(branch: Reactive<PatchSeries>) {
	const isIntegrated = $derived(branch.current.integrated);

	const [
		projectService,
		appState,
		cloudProjectService,
		latestBranchLookupService,
		cloudBranchService
	] = inject(
		ProjectService,
		AppState,
		CloudProjectService,
		LatestBranchLookupService,
		CloudBranchService,
		WebRoutesService
	);
	const project = readableToReactive(projectService.project);

	const cloudProject = $derived(
		project.current?.api?.repository_id
			? getProjectByRepositoryId(appState, cloudProjectService, project.current.api.repository_id)
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
		if (!isIntegrated) return;
		if (!isFound(cloudBranch?.current)) return;
		if (cloudBranch.current.value.status === BranchStatus.Closed) return;

		cloudBranchService.updateBranch(cloudBranch.current.id, {
			status: BranchStatus.Closed
		});
	});
}

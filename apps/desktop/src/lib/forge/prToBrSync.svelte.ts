import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import { ProjectService } from '$lib/project/projectService';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
import { inject } from '@gitbutler/shared/context';
import { isFound, map } from '@gitbutler/shared/network/loadable';
import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { AppState } from '@gitbutler/shared/redux/store.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function syncPrToBr(
	prNumber: Reactive<number | undefined>,
	reviewId: Reactive<string | undefined>
) {
	const [projectService, appState, latestBranchLookupService, cloudBranchService, forge] = inject(
		ProjectService,
		AppState,
		LatestBranchLookupService,
		CloudBranchService,
		DefaultForgeFactory
	);

	const prNumber2 = $derived(prNumber.current);
	const prResult = $derived(prNumber2 ? forge.current.prService?.get(prNumber2) : undefined);
	const pr = $derived(prResult?.current.data);
	const project = readableToReactive(projectService.project);

	const cloudProject = $derived(
		project.current?.api?.repository_id
			? getProjectByRepositoryId(project.current.api.repository_id)
			: undefined
	);

	const cloudBranchUuid = $derived(
		map(cloudProject?.current, (cloudProject) => {
			if (!reviewId.current) return;

			return lookupLatestBranchUuid(
				appState,
				latestBranchLookupService,
				cloudProject.owner,
				cloudProject.slug,
				reviewId.current
			);
		})
	);

	const cloudBranch = $derived(
		map(cloudBranchUuid?.current, (cloudBranchUuid) => {
			return getBranchReview(cloudBranchUuid);
		})
	);

	$effect(() => {
		if (!project.current?.api) return;
		if (!pr) return;
		if (!isFound(cloudBranch?.current)) return;
		if (cloudBranch.current.value.forgeUrl) return;

		cloudBranchService.updateBranch(cloudBranch.current.id, {
			forgeUrl: pr.htmlUrl,
			forgeDescription: `#${pr.number}`
		});
	});
}

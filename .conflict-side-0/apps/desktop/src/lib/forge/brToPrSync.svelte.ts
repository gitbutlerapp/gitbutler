import { getPr } from '$lib/forge/getPr.svelte';
import { getForgePrService } from '$lib/forge/interface/forgePrService';
import {
	formatButRequestDescription,
	updateButRequestPrDescription
} from '$lib/forge/shared/prFooter';
import { ProjectService } from '$lib/project/projectService';
import { BranchService as CloudBranchService } from '@gitbutler/shared/branches/branchService';
import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
import { inject } from '@gitbutler/shared/context';
import { combine, map } from '@gitbutler/shared/network/loadable';
import { ProjectService as CloudProjectService } from '@gitbutler/shared/organizations/projectService';
import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { AppState } from '@gitbutler/shared/redux/store.svelte';
import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
import type { PatchSeries } from '$lib/branches/branch';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function syncBrToPr(branch: Reactive<PatchSeries>) {
	const pr = getPr(branch);

	const [
		projectService,
		appState,
		cloudProjectService,
		latestBranchLookupService,
		cloudBranchService,
		webRoutes
	] = inject(
		ProjectService,
		AppState,
		CloudProjectService,
		LatestBranchLookupService,
		CloudBranchService,
		WebRoutesService
	);
	const prService = readableToReactive(getForgePrService());
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

	const contributors = $derived(
		map(cloudBranch?.current, (cloudBranch) =>
			cloudBranch.contributors.map((contributor) => contributor.email)
		)
	);
	const butlerRequestUrl = $derived(
		map(combine([cloudBranch?.current, cloudProject?.current]), ([cloudBranch, cloudProject]) => {
			return webRoutes.projectReviewBranchUrl({
				branchId: cloudBranch.branchId,
				projectSlug: cloudProject.slug,
				ownerSlug: cloudProject.owner
			});
		})
	);
	const prBody = $derived(pr.current?.body);
	const prNumber = $derived(pr.current?.number);
	const bodyChanged = $derived.by(() => {
		if (!prBody || !butlerRequestUrl || !contributors) return false;

		const formattedBody = formatButRequestDescription(prBody, butlerRequestUrl, contributors);
		return formattedBody === prBody;
	});

	$effect(() => {
		if (
			!bodyChanged ||
			!prBody ||
			!prNumber ||
			!butlerRequestUrl ||
			!contributors ||
			!prService.current
		)
			return;

		updateButRequestPrDescription(
			prService.current,
			prNumber,
			prBody,
			butlerRequestUrl,
			contributors
		);
	});
}

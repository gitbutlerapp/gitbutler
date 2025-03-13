import { BrToPrService } from '$lib/forge/shared/prFooter';
import { ProjectService } from '$lib/project/projectService';
import { getContext } from '@gitbutler/shared/context';
import { registerInterest } from '@gitbutler/shared/interest/registerInterestFunction.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { PatchSeries } from '$lib/branches/branch';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function syncBrToPr(branch: Reactive<PatchSeries>) {
	const projectService = getContext(ProjectService);
	const project = readableToReactive(projectService.project);
	const brToPrService = getContext(BrToPrService);

	$effect(() => {
		if (!branch.current?.prNumber) return;
		if (!branch.current?.reviewId) return;
		if (!project.current?.api?.repository_id) return;

		const interest = brToPrService.updateButRequestPrDescription(
			branch.current.prNumber,
			branch.current.reviewId,
			project.current.api.repository_id
		);
		registerInterest(interest);
	});
}

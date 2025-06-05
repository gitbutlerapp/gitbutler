import { BrToPrService } from '$lib/forge/shared/prFooter';
import { ProjectService } from '$lib/project/projectService';
import { getContext } from '@gitbutler/shared/context';
import { registerInterest } from '@gitbutler/shared/interest/registerInterestFunction.svelte';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function syncBrToPr(
	prNumber: Reactive<number | undefined>,
	reviewId: Reactive<string | undefined>
) {
	const projectService = getContext(ProjectService);
	const project = readableToReactive(projectService.project);
	const brToPrService = getContext(BrToPrService);

	$effect(() => {
		if (!prNumber.current) return;
		if (!reviewId.current) return;
		if (!project.current?.api?.repository_id) return;

		const interest = brToPrService.updateButRequestPrDescription(
			prNumber.current,
			reviewId.current,
			project.current.api.repository_id
		);
		registerInterest(interest);
	});
}

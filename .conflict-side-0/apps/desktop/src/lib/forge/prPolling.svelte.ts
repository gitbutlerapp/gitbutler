import { updateStalePrSelection, type UiState } from '$lib/state/uiState.svelte';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import type { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import type { PullRequest } from '$lib/forge/interface/types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

const POLLING_INTERVAL = 15 * 60 * 1000; // 15 minutes.

/**
 * Return a reactive list of pull requests for the given project ID.
 *
 * This will poll the PRs in the defined interval, and update the everytime the project ID changes.
 * This will also update the branch selection. If a PR is selected, it will check if the PR still exists.
 */
export default function prList(
	projectId: Reactive<string>,
	forge: DefaultForgeFactory,
	uiState: UiState
): Reactive<PullRequest[]> {
	const prListResult = $derived(
		forge.current.listService?.list(projectId.current, POLLING_INTERVAL)
	);

	const prList = $derived(prListResult?.current.data ?? []);

	$effect(() => {
		updateStalePrSelection(uiState, projectId.current, prList);
	});

	return reactive(() => prList);
}

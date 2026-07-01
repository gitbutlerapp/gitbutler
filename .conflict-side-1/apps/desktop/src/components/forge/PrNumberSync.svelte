<script lang="ts">
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { LISTING_SERVICE } from "$lib/forge/listingService.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const listingService = inject(LISTING_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	// Gate on the listService capability rather than a forge name: any
	// forge that can list reviews can have its number persisted to the
	// branch (GitHub + GitLab today).
	const canListReviews = $derived(!!forgeInfoQuery.response?.capabilities.listService);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const listedPrQuery = $derived(listingService.getByBranch(projectId, branchName));
	const listedPrNumber = $derived(listedPrQuery.response?.number);

	let hasRun = false;
	$effect(() => {
		if (canListReviews && listedPrNumber && !hasRun) {
			hasRun = true;
			stackService.updateBranchPrNumber({
				projectId: projectId,
				stackId,
				branchName,
				prNumber: listedPrNumber,
			});
		}
	});
</script>

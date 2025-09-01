<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const forgeListing = $derived(forge.current.listService);
	const forgeName = $derived(forge.current.name);
	const listedPrResult = $derived(forgeListing?.getByBranch(projectId, branchName));

	const listedPrNumber = $derived(listedPrResult?.current.data?.number);

	let hasRun = false;
	$effect(() => {
		if (forgeName === 'github' && listedPrNumber && !hasRun) {
			hasRun = true;
			stackService.updateBranchPrNumber({
				projectId: projectId,
				stackId,
				branchName,
				prNumber: listedPrNumber
			});
		}
	});
</script>

<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();
	const [stackService, forge] = inject(StackService, DefaultForgeFactory);

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

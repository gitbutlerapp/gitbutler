<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import PushButton from '$components/v3/PushButton.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId: unsyncedStackId }: Props = $props();
	const [stackService] = inject(StackService);
	const [stackId, branchesResult] = $derived([
		unsyncedStackId,
		stackService.branches(projectId, unsyncedStackId)
	]);
</script>

<ReduxResult result={branchesResult.current}>
	{#snippet children(branches)}
		{#each branches as branch, i}
			{@const first = i === 0}
			{@const last = i === branches.length - 1}
			<BranchCard {projectId} {stackId} branchName={branch.name} {first} {last} />
		{/each}
		<PushButton {projectId} {stackId} multipleBranches={branches.length > 0} />
	{/snippet}
</ReduxResult>

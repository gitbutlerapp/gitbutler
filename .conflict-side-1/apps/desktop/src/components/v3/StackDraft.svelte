<script lang="ts">
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [uiState, stackService] = inject(UiState, StackService);
	const draftBranchName = $derived(uiState.global.draftBranchName);

	// Automatic branch name suggested by back end.
	let newName = $state('');

	const newNameResult = stackService.newBranchName(projectId);
	newNameResult.then((name) => {
		newName = name.data || '';
	});

	$effect(() => {
		if (newName && !draftBranchName.current) {
			draftBranchName.set(newName);
		}
	});

	const branchName = $derived(draftBranchName.current || newName);
</script>

<div class="stack-draft">
	<BranchCard type="draft-branch" {projectId} {branchName}>
		{#snippet header()}
			<BranchHeader
				type="draft-branch"
				{branchName}
				{projectId}
				iconName="branch-local"
				readonly={false}
				lineColor="var(--clr-commit-local)"
			/>
		{/snippet}
	</BranchCard>
	<CommitGoesHere selected draft />
</div>

<style lang="postcss">
	.stack-draft {
		padding: 12px;
	}
</style>

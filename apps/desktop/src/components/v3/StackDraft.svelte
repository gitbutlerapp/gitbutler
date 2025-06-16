<script lang="ts">
	import BranchCard from '$components/v3/BranchCard.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [uiState, stackService] = inject(UiState, StackService);
	const draftBranchName = $derived(uiState.global.draftBranchName);
	const projectState = $derived(uiState.project(projectId));

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

<div
	data-testid={TestId.StackDraft}
	class="draft-stack"
	style:width={uiState.global.stackWidth.current + 'rem'}
>
	<div class="new-commit-view" data-testid={TestId.NewCommitView}>
		<NewCommitView
			{projectId}
			noDrawer
			onclose={() => projectState.exclusiveAction.set(undefined)}
		/>
	</div>
	<BranchCard
		type="draft-branch"
		{projectId}
		{branchName}
		readonly={false}
		lineColor="var(--clr-commit-local)"
	>
		{#snippet branchContent()}
			<CommitGoesHere selected draft />
		{/snippet}
	</BranchCard>
</div>

<style lang="postcss">
	.draft-stack {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		padding: 12px;
	}
	.new-commit-view {
		margin-bottom: 12px;
		padding: 12px;
	}
</style>

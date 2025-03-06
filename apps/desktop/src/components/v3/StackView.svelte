<script lang="ts">
	import BranchList from '$components/v3/BranchList.svelte';
	import ResizeableSplitLayout from '$components/v3/ResizeableSplitLayout.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		selectedBranchName: string;
		selectedCommitId?: string;
		children: Snippet;
	}

	const { stackId, projectId, selectedBranchName, selectedCommitId, children }: Props = $props();
</script>

<ResizeableSplitLayout {projectId}>
	{#snippet left()}
		<BranchList {projectId} {stackId} {selectedBranchName} {selectedCommitId} />
	{/snippet}
	{#snippet main()}
		{@render children()}
	{/snippet}
</ResizeableSplitLayout>

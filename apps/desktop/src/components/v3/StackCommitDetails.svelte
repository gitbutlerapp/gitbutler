<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		selectedCommitId: string | undefined;
	}

	let { selectedCommitId = $bindable() }: Props = $props();

	$inspect('stackCommitDetails.selectedCommit', selectedCommitId);
</script>

<div class="wrapper">
	<div class="header">
		<Button
			class=""
			kind="ghost"
			size="tag"
			icon="cross"
			onclick={() => (selectedCommitId = undefined)}
		/>
	</div>
	<div class="body">
		<div>{selectedCommitId}</div>
	</div>
</div>

<style>
	.wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;

		background-color: white;
	}
	.header {
		display: flex;
		justify-content: flex-end;
		padding: 2px 4px;
	}
	.body {
		display: flex;
		flex-direction: column;
		padding: 4px;
	}
</style>

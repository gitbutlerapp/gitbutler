<script lang="ts">
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import CardOverlay from '$lib/components/Dropzone/CardOverlay.svelte';
	import Dropzone from '$lib/components/Dropzone/Dropzone.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { Branch } from '$lib/vbranches/types';
	import type { Snippet } from 'svelte';

	const branchDragActionsFactory = getContext(BranchDragActionsFactory);
	const branch = getContextStore(Branch);

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const actions = $derived(branchDragActionsFactory.build($branch));
</script>

<div class="commit-list-item">
	<div class="commit-card-wrapper">
		{@render moveCommitDropzone()}
	</div>
</div>

<!-- We require the dropzones to be nested -->
{#snippet moveCommitDropzone()}
	<Dropzone
		accepts={actions.acceptMoveCommit.bind(actions)}
		ondrop={actions.onMoveCommit.bind(actions)}
		fillHeight
	>
		{@render branchDropDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet branchDropDropzone()}
	<Dropzone
		accepts={actions.acceptBranchDrop.bind(actions)}
		ondrop={actions.onBranchDrop.bind(actions)}
		fillHeight
	>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
	</Dropzone>
{/snippet}

<style>
	.commit-list-item {
		display: flex;
		position: relative;
		padding: 0;
		gap: 8px;
		flex-grow: 1;
		overflow: hidden;
		&:last-child {
			padding-bottom: 0;
		}
	}
	.commit-card-wrapper {
		position: relative;
		width: 100%;
	}
</style>

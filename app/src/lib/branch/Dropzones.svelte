<script lang="ts">
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import CardOverlay from '$lib/dropzone/CardOverlay.svelte';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
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

<div class="dragzone-wrapper">
	{@render moveCommitDropzone()}
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
	.dragzone-wrapper {
		display: flex;
		flex-direction: column;
		position: relative;
		flex-grow: 1;
		width: 100%;
		/* overflow: hidden; */
	}
</style>

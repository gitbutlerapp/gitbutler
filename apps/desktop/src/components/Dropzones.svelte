<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import { BranchDragActionsFactory } from '$lib/branches/dragActions';
	import { BranchStack } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	const branchDragActionsFactory = getContext(BranchDragActionsFactory);
	const stack = getContextStore(BranchStack);

	interface Props {
		children: Snippet;
		type?: 'commit' | 'file' | 'all';
	}

	const { children, type = 'all' }: Props = $props();

	const actions = $derived(branchDragActionsFactory.build($stack));

	const commitTypes: Props['type'][] = ['commit', 'all'];
	function acceptsCommits(dropData: unknown) {
		if (!commitTypes.includes(type)) {
			return false;
		}
		return actions.acceptMoveCommit(dropData);
	}

	const fileTypes: Props['type'][] = ['file', 'all'];
	function acceptsFiles(dropData: unknown) {
		if (!fileTypes.includes(type)) {
			return false;
		}
		return actions.acceptBranchDrop(dropData);
	}
</script>

<div class="dragzone-wrapper">
	{@render moveCommitDropzone()}
</div>

<!-- We require the dropzones to be nested -->
{#snippet moveCommitDropzone()}
	<Dropzone accepts={acceptsCommits} ondrop={actions.onMoveCommit.bind(actions)} fillHeight>
		{@render branchDropDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Move here" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet branchDropDropzone()}
	<Dropzone accepts={acceptsFiles} ondrop={actions.onBranchDrop.bind(actions)} fillHeight>
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
	}
</style>

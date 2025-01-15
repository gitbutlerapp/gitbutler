<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { CommitDragActions, CommitDragActionsFactory } from '$lib/commits/dragActions';
	import { Commit, DetailedCommit } from '$lib/vbranches/types';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	const commitDragActionsFactory = getContext(CommitDragActionsFactory);

	interface Props {
		commit: DetailedCommit | Commit;
		children: Snippet;
	}

	const { commit, children }: Props = $props();

	const stack = maybeGetContextStore(BranchStack);

	const actions = $derived<CommitDragActions | undefined>(
		$stack && commitDragActionsFactory.build($stack, commit)
	);
</script>

<div class="dropzone-wrapper">
	{#if actions}
		{@render ammendDropzone()}
	{:else}
		{@render children()}
	{/if}
</div>

<!-- We require the dropzones to be nested -->
{#snippet ammendDropzone()}
	<Dropzone accepts={actions!.acceptsAmend.bind(actions)} ondrop={actions!.onAmend.bind(actions)}>
		{@render squashDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Amend commit" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet squashDropzone()}
	<Dropzone accepts={actions!.acceptsSquash.bind(actions)} ondrop={actions!.onSquash.bind(actions)}>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Squash commit" />
		{/snippet}
	</Dropzone>
{/snippet}

<style>
	.dropzone-wrapper {
		position: relative;
		width: 100%;
	}
</style>

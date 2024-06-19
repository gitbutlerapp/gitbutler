<script lang="ts">
	import { getContext, maybeGetContextStore } from '$lib/utils/context';
	import { RemoteCommit, Branch, Commit } from '$lib/vbranches/types';
	import Dropzone from '$lib/components/Dropzone/Dropzone.svelte';
	import type { Snippet } from 'svelte';
	import CardOverlay from '$lib/components/Dropzone/CardOverlay.svelte';
	import { CommitDragActionsFactory } from '$lib/commits/dragActions';

	const commitDragActionsFactory = getContext(CommitDragActionsFactory);

	interface Props {
		commit: Commit | RemoteCommit;
		children: Snippet;
	}

	const { commit, children }: Props = $props();

	const branch = maybeGetContextStore(Branch);

	const actions = $derived.by(() => {
		if (!$branch) return;

		return commitDragActionsFactory.build($branch, commit);
	});
</script>

<div class="commit-list-item">
	<div class="commit-card-wrapper">
		{#if actions}
			{@render ammendDropzone()}
		{:else}
			{@render children()}
		{/if}
	</div>
</div>

<!-- We require the dropzones to be nested -->
{#snippet ammendDropzone()}
	<Dropzone accepts={actions!.acceptAmend.bind(actions)} ondrop={actions!.onAmend.bind(actions)}>
		{@render squashDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Ammend commit" />
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet squashDropzone()}
	<Dropzone accepts={actions!.acceptSquash.bind(actions)} ondrop={actions!.onSquash.bind(actions)}>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay {hovered} {activated} label="Squash commit" />
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

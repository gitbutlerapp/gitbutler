<script lang="ts">
	import { CommitDragActions, CommitDragActionsFactory } from '$lib/commits/dragActions';
	import { stackingFeature } from '$lib/config/uiFeatureFlags';
	import CardOverlay from '$lib/dropzone/CardOverlay.svelte';
	import Dropzone from '$lib/dropzone/Dropzone.svelte';
	import { Commit, VirtualBranch, DetailedCommit } from '$lib/vbranches/types';
	import { getContext, maybeGetContextStore } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	const commitDragActionsFactory = getContext(CommitDragActionsFactory);

	interface Props {
		commit: DetailedCommit | Commit;
		children: Snippet;
	}

	const { commit, children }: Props = $props();

	const branch = maybeGetContextStore(VirtualBranch);

	const actions = $derived<CommitDragActions | undefined>(
		$branch && commitDragActionsFactory.build($branch, commit)
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
	<Dropzone accepts={actions!.acceptAmend.bind(actions)} ondrop={actions!.onAmend.bind(actions)}>
		{@render squashDropzone()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay
				{hovered}
				{activated}
				label="Amend commit"
				extraPaddings={$stackingFeature
					? {
							left: 4
						}
					: {
							left: 4
						}}
			/>
		{/snippet}
	</Dropzone>
{/snippet}

{#snippet squashDropzone()}
	<Dropzone accepts={actions!.acceptSquash.bind(actions)} ondrop={actions!.onSquash.bind(actions)}>
		{@render children()}

		{#snippet overlay({ hovered, activated })}
			<CardOverlay
				{hovered}
				{activated}
				label="Squash commit"
				extraPaddings={$stackingFeature
					? {
							left: -4
						}
					: {
							left: 4
						}}
			/>
		{/snippet}
	</Dropzone>
{/snippet}

<style>
	.dropzone-wrapper {
		position: relative;
		width: 100%;
	}
</style>

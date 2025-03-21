<script lang="ts">
	import StackTabs from '$components/v3/StackTabs.svelte';
	import WorkspaceView from '$components/v3/WorkspaceView.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { getContext } from '@gitbutler/shared/context';
	import type { PageData } from './$types';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const { data, children }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);

	/** Offset width for tabs component. */
	let width = $state<number>();

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});
</script>

{#if projectId}
	<WorkspaceView {projectId} {stackId}>
		{#snippet right({ viewportWidth })}
			<StackTabs {projectId} selectedId={stackId} bind:width />
			<div class="contents" class:rounded={width === viewportWidth}>
				{@render children()}
			</div>
		{/snippet}
	</WorkspaceView>
{/if}

<style>
	.contents {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;

		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		border: 1px solid var(--clr-border-2);

		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;

		&.rounded {
			border-radius: 0 var(--radius-ml) var(--radius-ml) var(--radius-ml);
		}
	}
</style>

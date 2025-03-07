<script lang="ts">
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
	const branchName = $derived(page.params.branchName);

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});
</script>

{#if projectId}
	<WorkspaceView {projectId} {stackId} {branchName}>
		{@render children()}
	</WorkspaceView>
{/if}

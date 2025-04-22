<script lang="ts">
	import WorkspaceView from '$components/v3/WorkspaceView.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import type { PageData } from './$types';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const stackService = getContext(StackService);
	const settingsService = getContext(SettingsService);
	const modeService = getContext(ModeService);
	const settingsStore = settingsService.appSettings;
	const mode = modeService.mode;

	const { data, children }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(page.params.projectId!);
	const stackId = $derived(page.params.stackId);

	const stacks = $derived(stackService.stacks(projectId));

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});

	$effect(() => {
		// If the data is loading, do nothing
		if (!stacks?.current.data) return;
		const stackFoundWithCurrentPageId = stacks?.current.data?.some((stack) => stack.id === stackId);
		// If we are on a valid stack, do nothing
		if (stackFoundWithCurrentPageId) return;

		if (stacks.current.data.length === 0) {
			goto(`/${data.projectId}/workspace`);
		} else {
			goto(`/${data.projectId}/workspace/${stacks.current.data[0]!.id}`);
		}
	});

	function gotoEdit() {
		goto(`/${projectId}/edit`);
	}

	$effect(() => {
		if ($mode?.type === 'Edit') {
			// That was causing an incorrect linting error when project.id was accessed inside the reactive block
			gotoEdit();
		}
	});
</script>

<WorkspaceView {projectId} {stackId}>
	{#snippet stack()}
		{@render children()}
	{/snippet}
</WorkspaceView>

<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import WorkspaceView from '$components/v3/WorkspaceView.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { ModeService } from '$lib/mode/modeService';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import type { PageData } from './$types';
	import type { Snippet } from 'svelte';

	const settingsService = getContext(SettingsService);
	const modeService = getContext(ModeService);
	const settingsStore = settingsService.appSettings;
	const mode = modeService.mode;

	const { data }: { data: PageData; children: Snippet } = $props();

	const projectId = $derived(page.params.projectId!);
	const [uiState, stackService] = inject(UiState, StackService);
	const projectState = $derived(uiState.project(projectId));
	const stackId = $derived(projectState.stackId.current);
	const stackResult = $derived(projectId ? stackService.stacks(projectId) : undefined);
	const firstStackId = $derived(stackResult?.current.data?.at(0)?.id);

	// TODO: Is there a better way we can support testing?
	const stackIdParam = $derived(page.url.searchParams.get('stackId'));
	$effect(() => {
		if (!stackIdParam && stackId) {
			goto(stackPath(projectId, stackId));
		} else if (!stackIdParam && firstStackId) {
			goto(stackPath(projectId, firstStackId));
		}
		if (stackIdParam && stackIdParam !== stackId) {
			projectState.stackId.set(stackIdParam);
		}
	});

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if ($settingsStore && !$settingsStore.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
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

<WorkspaceView {projectId} {stackId} />

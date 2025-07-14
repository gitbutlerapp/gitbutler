<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import WorkspaceView from '$components/WorkspaceView.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import { QueryStatus } from '@reduxjs/toolkit/query';

	const modeService = getContext(ModeService);

	const mode = modeService.mode;

	const projectId = $derived(page.params.projectId!);
	const [uiState, stackService] = inject(UiState, StackService);
	const projectState = $derived(uiState.project(projectId));
	const stackId = $derived(projectState.stackId.current);

	const firstStackResult = $derived(stackService.stackAt(projectId, 0));
	const firstStack = $derived(firstStackResult.current.data);
	const stackResult = $derived(stackId ? stackService.stackById(projectId, stackId) : undefined);

	$effect(() => {
		if (stackId === undefined && firstStack) {
			projectState.stackId.set(firstStack.id);
		} else if (
			stackId &&
			stackResult?.current.status === QueryStatus.fulfilled &&
			stackResult.current.data === undefined
		) {
			projectState.stackId.set(undefined);
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

<WorkspaceView {projectId} />

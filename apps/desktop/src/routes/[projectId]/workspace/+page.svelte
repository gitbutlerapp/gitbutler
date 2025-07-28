<script lang="ts">
	import WorkspaceView from './WorkspaceView.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { QueryStatus } from '@reduxjs/toolkit/query';

	const modeService = inject(MODE_SERVICE);

	const projectId = $derived(page.params.projectId!);
	const mode = $derived(modeService.mode({ projectId }));
	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
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
		if (mode.current.data?.type === 'Edit') {
			// That was causing an incorrect linting error when project.id was accessed inside the reactive block
			gotoEdit();
		}
	});
</script>

<WorkspaceView {projectId} />

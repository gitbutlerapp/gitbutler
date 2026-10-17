<script lang="ts">
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const stack = $derived(stackService.stackById(projectId, "stack-1").response);
	const selectedCommitId = $derived(uiState.lane("stack-1").selection.current?.commitId);
	const details = $derived(
		stack && selectedCommitId ? stackService.commitChanges(projectId, selectedCommitId) : undefined,
	);
</script>

{#if details}{details.result.status}{/if}

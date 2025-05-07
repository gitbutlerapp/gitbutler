<script lang="ts">
	import { page } from '$app/state';
	import StackView from '$components/v3/StackView.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	const projectId = $derived(page.params.projectId!);
	const stackId = $derived(page.params.stackId!);

	const [uiState] = inject(UiState);

	const projectState = $derived(uiState.project(projectId));

	$effect(() => {
		// TODO: Remove after multi-stack view transition complete.
		projectState.stackId.set(stackId);
	});
</script>

<StackView {projectId} {stackId} />

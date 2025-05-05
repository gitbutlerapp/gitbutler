<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		children: Snippet;
	};

	const { projectId, children }: Props = $props();

	const [uiState] = inject(UiState);

	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(projectState.stackId);
</script>

<div class="stack-card" class:selected>
	{@render children()}
</div>

<style>
	.stack-card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 100%;
	}
</style>

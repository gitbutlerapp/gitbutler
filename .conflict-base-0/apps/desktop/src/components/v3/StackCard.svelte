<script lang="ts">
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		stackName: string;
		children: Snippet;
		contextMenu: Snippet;
	};

	const { projectId, children, contextMenu }: Props = $props();

	const [uiState] = inject(UiState);

	const projectState = $derived(uiState.project(projectId));
	const selected = $derived(projectState.stackId);
</script>

<div class="stack-card text-15 text-bold text-" class:selected>
	<div class="header">
		{@render contextMenu()}
	</div>
	<div class="content">
		{@render children()}
	</div>
</div>

<style>
	.stack-card {
		display: flex;
		flex-direction: column;
		gap: 6px;
		width: 100%;

		&:hover .header {
			opacity: 0.5;
		}
	}

	.header {
		display: flex;
		color: var(--clr-text-2);
		justify-content: flex-end;
		opacity: 0;
		padding: 0 6px;
		&:hover {
			opacity: 1;
		}
		--menu-btn-size: 20px;
	}
</style>

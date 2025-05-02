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
	<div class="content">
		<div class="header">
			<div>
				{@render contextMenu()}
			</div>
		</div>
		{@render children()}
	</div>
</div>

<style>
	.stack-card {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
	}

	.header {
		display: flex;
		color: var(--clr-text-2);
		justify-content: space-between;
		padding-right: 6px;
		--menu-btn-size: 20px;
	}
</style>

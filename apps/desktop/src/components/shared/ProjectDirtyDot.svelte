<script lang="ts">
	import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Tooltip } from "@gitbutler/ui";

	interface Props {
		projectId: string;
	}

	const { projectId }: Props = $props();

	const worktreeService = inject(WORKTREE_SERVICE);
	const hasChangesQuery = $derived(worktreeService.hasChanges(projectId));
	const hasChanges = $derived(hasChangesQuery.response ?? false);
</script>

{#if hasChanges}
	<Tooltip text="Has uncommitted changes">
		<div class="dirty-dot"></div>
	</Tooltip>
{/if}

<style>
	.dirty-dot {
		flex-shrink: 0;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background-color: var(--clr-pop-60);
	}
</style>

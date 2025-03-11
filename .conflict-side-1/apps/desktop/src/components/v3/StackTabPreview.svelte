<script lang="ts">
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		count: number;
		selected: boolean;
	};
	const { projectId, count, selected }: Props = $props();

	async function addNew() {
		goto(workspacePath(projectId));
	}
</script>

<button aria-label="new-stack" type="button" class="new-stack" onclick={() => addNew()}>
	{#if selected}
		<div class="selected-accent"></div>
	{/if}
	({count})
</button>

<style lang="postcss">
	.new-stack {
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: 10px 10px 0 0;
		border-right: none;
		padding: 14px 20px;
		position: relative;
		overflow: hidden;
		&:hover {
			background: var(--clr-stack-tab-active);
		}
	}

	.selected-accent {
		position: absolute;
		background: var(--clr-theme-pop-element);
		width: 100%;
		height: 3px;
		left: 0;
		top: 0;
	}
</style>

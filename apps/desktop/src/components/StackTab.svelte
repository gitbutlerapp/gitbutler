<script lang="ts">
	import { stackPath } from '$lib/routes/routes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Tab } from '$lib/tabs/tab';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		tab: Tab;
		first: boolean;
		last: boolean;
		selected: boolean;
	};

	const { projectId, tab, first, last, selected }: Props = $props();
</script>

<button
	onclick={() => goto(stackPath(projectId, tab.id))}
	class="tab"
	class:first
	class:last
	class:selected
	type="button"
>
	{#if selected}
		<div class="selected-accent"></div>
	{/if}
	<div class="icon">
		{#if tab.anchors.length > 0}
			<Icon name="chain-link" verticalAlign="top" />
		{:else}
			<Icon name="branch-small" verticalAlign="top" />
		{/if}
	</div>
	<div class="name">
		{tab.name}
	</div>
</button>

<style lang="postcss">
	.tab {
		display: flex;
		align-items: center;
		gap: 8px;
		position: relative;
		padding: 12px 14px;
		background: var(--clr-stack-tab-inactive);
		border: 1px solid var(--clr-border-2);
		border-right: none;
		border-bottom: none;
		overflow: hidden;

		&.first {
			border-radius: var(--radius-ml) 0 0 0;
		}
	}

	.icon {
		color: var(--clr-text-2);
		display: inline-block;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		width: 18px;
		height: 18px;
		line-height: 16px;
	}

	.name {
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}

	.selected {
		background-color: var(--clr-stack-tab-active);
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

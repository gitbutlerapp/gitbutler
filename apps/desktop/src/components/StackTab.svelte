<script lang="ts">
	import { stackPath } from '$lib/routes/routes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Tab } from '$lib/tabs/tab';
	import { goto } from '$app/navigation';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';

	type Props = {
		projectId: string;
		tab: Tab;
		first: boolean;
		last: boolean;
		selected: boolean;
	};

	let kebabMenuTrigger = $state<HTMLElement>();
	let contextMenuEl = $state<ContextMenu>();
	let isContextMenuOpen = $state(false);

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
	<div class="tab__overflow-menu">
		<Button
			kind="ghost"
			icon="kebab"
			bind:el={kebabMenuTrigger}
			onclick={() => {
				contextMenuEl?.toggle();
			}}
			activated={isContextMenuOpen}
		></Button>
	</div>

	<ContextMenu
		bind:this={contextMenuEl}
		leftClickTrigger={kebabMenuTrigger}
		ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
		side="bottom"
		horizontalAlign="left"
	>
		<ContextMenuSection>
			<ContextMenuItem
				label="Unapply Stack"
				keyboardShortcut="$mod+X"
				onclick={() => {
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Rename"
				keyboardShortcut="$mod+R"
				onclick={() => {
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
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

		.tab__overflow-menu {
			opacity: 0;
			width: 0px;
			transition:
				opacity 100ms ease-in,
				width 100ms ease-in-out;
		}

		&:active .tab__overflow-menu,
		&:hover .tab__overflow-menu,
		&:focus .tab__overflow-menu {
			margin-right: 8px;
			display: flex;
			opacity: 1;
			width: 16px;
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

<script lang="ts">
	import { stackPath } from '$lib/routes/routes.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Tab } from '$lib/tabs/tab';

	type Props = {
		projectId: string;
		tab: Tab;
		first: boolean;
		last: boolean;
		selected: boolean;
	};

	const { projectId, tab, first, last, selected }: Props = $props();

	let kebabMenuTrigger = $state<HTMLElement>();
	let contextMenu = $state<ContextMenu>();
	let isContextMenuOpen = $state(false);
</script>

<a
	data-sveltekit-keepfocus
	href={stackPath(projectId, tab.id)}
	class="tab"
	class:first
	class:last
	class:selected
	class:menu-open={isContextMenuOpen}
>
	<div class="icon">
		<Icon name={tab.anchors.length > 0 ? 'chain-link' : 'branch-small'} verticalAlign="top" />
	</div>
	<div class="content">
		<div class="text-12 text-semibold name">
			{tab.name}
		</div>
		<div class="menu-button-wrap">
			<button
				class="menu-button"
				class:menu-open={isContextMenuOpen}
				onclick={(e) => {
					e.preventDefault();
					e.stopPropagation();
					contextMenu?.toggle();
				}}
				bind:this={kebabMenuTrigger}
				type="button"
			>
				<Icon name="kebab" />
			</button>
		</div>
	</div>
</a>

<ContextMenu
	bind:this={contextMenu}
	leftClickTrigger={kebabMenuTrigger}
	ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
	side="bottom"
>
	<ContextMenuSection>
		<ContextMenuItem
			label="Unapply Stack"
			keyboardShortcut="$mod+X"
			onclick={() => {
				contextMenu?.close();
			}}
		/>
		<ContextMenuItem
			label="Rename"
			keyboardShortcut="$mod+R"
			onclick={() => {
				contextMenu?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.tab {
		--menu-btn-size: 20px;

		display: flex;
		align-items: center;
		position: relative;
		padding: 0 12px 0 14px;
		height: 48px;
		background: var(--clr-stack-tab-inactive);
		border-right: 1px solid var(--clr-border-2);
		overflow: hidden;
		min-width: 100px;
		scroll-snap-align: start;

		&::after {
			content: '';
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 2px;
			transform: translateY(-100%);
			transition: transform var(--transition-fast);
		}
	}
	.first {
		border-radius: var(--radius-ml) 0 0 0;
	}
	.last {
		border-right: none;
	}

	.content {
		display: flex;
		flex-grow: 1;
		align-items: center;
		overflow: hidden;
		position: relative;
	}

	.menu-button-wrap {
		position: relative;
		overflow: hidden;
		width: 0;
		/* background-color: aqua; */
	}

	.tab:hover .name,
	.menu-open .name {
		/* Shrinks name to make room for hover button. */
		width: calc(100% - var(--menu-btn-size));
	}

	.tab:hover .menu-button-wrap,
	.menu-open .menu-button-wrap {
		opacity: 1;
		width: var(--menu-btn-size);
		/* We want the container to not take up extra space. */
		margin-left: calc(var(--menu-btn-size) * -1);
		/* But still be visible where it would normally display. */
		transform: translateX(calc(var(--menu-btn-size) + 20%));
	}

	.menu-button {
		display: flex;
		color: var(--clr-text-2);
		padding: 4px 2px;
		margin-left: -2px;

		&.menu-open,
		&:hover {
			color: var(--clr-text-1);
		}
	}

	.tab:not(.selected):hover,
	.tab:not(.selected):focus-within {
		background: var(--clr-stack-tab-inactive-hover);
	}

	.tab:not(.selected):focus-within {
		&::after {
			transform: translateY(0);
			background: var(--clr-border-1);
		}
	}

	.selected {
		&::after {
			transform: translateY(0);
			background: var(--clr-theme-pop-element);
			z-index: var(--z-ground);
		}
	}

	.icon {
		color: var(--clr-text-2);
		display: flex;
		align-items: center;
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		border-radius: var(--radius-s);
		width: 16px;
		height: 16px;
		line-height: 16px;
		margin-right: 8px;
	}

	.name {
		width: 100%;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}

	.selected {
		background-color: var(--clr-stack-tab-active);
	}
</style>

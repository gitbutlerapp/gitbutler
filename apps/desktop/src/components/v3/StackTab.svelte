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

	let kebabMenuTrigger = $state<HTMLElement>();
	let contextMenu = $state<ContextMenu>();

	const { projectId, tab, first, last, selected }: Props = $props();
</script>

<div>
	<a
		data-sveltekit-keepfocus
		href={stackPath(projectId, tab.id)}
		class="tab"
		class:first
		class:last
		class:selected
	>
		<div class="icon">
			<Icon name={tab.anchors.length > 0 ? 'chain-link' : 'branch-small'} verticalAlign="top" />
		</div>
		<div class="content">
			<div class="text-12 text-semibold name">
				{tab.name}
			</div>
			<button
				class="menu-btn"
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
	</a>
</div>

<ContextMenu bind:this={contextMenu} leftClickTrigger={kebabMenuTrigger} side="bottom">
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
		display: flex;
		align-items: center;
		position: relative;
		padding: 0 14px;
		height: 48px;
		background: var(--clr-stack-tab-inactive);
		border-right: 1px solid var(--clr-border-2);
		overflow: hidden;
		flex: 0 0 auto;

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

		&.first {
			border-radius: var(--radius-ml) 0 0 0;
		}

		&.last {
			border-right: none;
		}

		.content {
			display: flex;
			width: 75px;
			align-items: center;
			overflow: hidden;
		}

		&:active .menu-btn,
		&:hover .menu-btn,
		&:focus-within .menu-btn {
			opacity: 1;
			min-width: 32px;
			padding: 8px;
		}

		.menu-btn {
			position: relative;
			opacity: 0;
			width: 0;
			overflow: hidden;

			display: flex;
			align-items: center;
			justify-content: center;
			color: var(--clr-text-2);

			&:hover {
				color: var(--clr-text-1);
			}
		}

		&:not(.selected):hover,
		&:not(.selected):focus-within {
			background: var(--clr-stack-tab-inactive-hover);
		}

		&:not(.selected):focus-within {
			&::after {
				transform: translateY(0);
				background: var(--clr-border-1);
			}
		}

		&.selected {
			&::after {
				transform: translateY(0);
				background: var(--clr-theme-pop-element);
				z-index: var(--z-ground);
			}
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

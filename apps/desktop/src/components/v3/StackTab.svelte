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
	let contextMenuEl = $state<ContextMenu>();
	let isContextMenuOpen = $state(false);
	let isHovered = $state(false);

	let nameEl = $state<HTMLDivElement>();
	let nameWidth = $state<number>();

	const { projectId, tab, first, last, selected }: Props = $props();

	$effect(() => {
		if (nameEl) {
			nameWidth = nameEl.offsetWidth - 1;
		}
	});
</script>

<div>
	<a href={stackPath(projectId, tab.id)} class="tab" class:first class:last class:selected>
		<div class="icon">
			{#if tab.anchors.length > 0}
				<Icon name="chain-link" verticalAlign="top" />
			{:else}
				<Icon name="branch-small" verticalAlign="top" />
			{/if}
		</div>
		<div class="tab__content" style:max-width="{nameWidth}px">
			<div class="text-12 text-semibold name" bind:this={nameEl}>
				{tab.name}
			</div>
			<div class={['tab__menu-btn-wrap', isContextMenuOpen || isHovered ? 'active' : '']}>
				<button
					class={['tab__menu-btn', isContextMenuOpen ? 'active' : '']}
					onmouseenter={() => (isHovered = true)}
					onmouseleave={() => (isHovered = false)}
					onclick={(e) => {
						e.preventDefault();
						e.stopPropagation();
						contextMenuEl?.toggle();
					}}
					bind:this={kebabMenuTrigger}
					type="button"
				>
					<Icon name="kebab" />
				</button>
			</div>
		</div>
	</a>
</div>

<ContextMenu
	bind:this={contextMenuEl}
	leftClickTrigger={kebabMenuTrigger}
	ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
	side="bottom"
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
		min-width: 80px;

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

		.tab__content {
			display: flex;
			align-items: center;
		}

		.tab__menu-btn-wrap {
			flex: 1;
			position: relative;
			display: flex;
			width: 0;
			overflow: hidden;
		}

		.tab__menu-btn {
			display: flex;
			align-items: center;
			justify-content: center;
			color: var(--clr-text-2);
			padding: 8px;

			&.active,
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

		.tab__menu-btn-wrap.active,
		&:active .tab__menu-btn-wrap,
		&:hover .tab__menu-btn-wrap,
		&:focus-within .tab__menu-btn-wrap {
			display: flex;
			opacity: 1;
			min-width: 32px;
			margin-right: -8px;

			.tab__menu-btn {
				opacity: 0.8;
			}
		}

		/* ACCENT LINES */
		&.selected {
			&::after {
				transform: translateY(0);
				background: var(--clr-theme-pop-element);
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
		box-sizing: content-box;
		text-overflow: ellipsis;
		white-space: nowrap;
		overflow: hidden;
	}

	.selected {
		background-color: var(--clr-stack-tab-active);
	}
</style>

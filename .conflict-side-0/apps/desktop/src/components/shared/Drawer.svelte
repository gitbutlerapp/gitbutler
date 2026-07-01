<script lang="ts">
	import AppScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import DrawerHeader from "$components/shared/DrawerHeader.svelte";
	import { persistWithExpiration } from "@gitbutler/shared/persisted";
	import { Icon } from "@gitbutler/ui";
	import { untrack } from "svelte";
	import { writable, type Writable } from "svelte/store";
	import type { Snippet } from "svelte";

	type Props = {
		header: Snippet<[HTMLDivElement]>;
		actions?: Snippet<[element: HTMLElement]>;
		closeActions?: Snippet;
		children: Snippet;
		testId?: string;
		collapsable?: boolean;
		persistId?: string;
		topBorder?: boolean;
		bottomBorder?: boolean;
		grow?: boolean;
		noshrink?: boolean;
		clientHeight?: number;
		defaultCollapsed?: boolean;
		notScrollable?: boolean;
		maxHeight?: string;
		transparent?: boolean;
		stickyHeader?: boolean;
		rounded?: boolean;
		reserveSpaceOnStuck?: boolean;
		closeButtonPlaceholder?: boolean;
		closeButtonPlaceholderWidth?: string;
		scrollRoot?: HTMLElement | null;
		highlighted?: boolean;
		onclose?: () => void;
		ontoggle?: (collapsed: boolean) => void;
	};

	let {
		header,
		actions,
		closeActions,
		children,
		testId,
		collapsable = true,
		persistId,
		bottomBorder = true,
		topBorder = false,
		grow,
		noshrink,
		clientHeight = $bindable(),
		defaultCollapsed = false,
		notScrollable = false,
		maxHeight,
		transparent,
		stickyHeader,
		rounded,
		reserveSpaceOnStuck,
		closeButtonPlaceholder,
		closeButtonPlaceholderWidth,
		scrollRoot,
		highlighted,
		ontoggle,
		onclose,
	}: Props = $props();

	let containerDiv = $state<HTMLDivElement>();
	let internalCollapsed: Writable<boolean | undefined> = untrack(() => persistId)
		? persistWithExpiration<boolean>(
				untrack(() => defaultCollapsed),
				untrack(() => persistId)!,
				1440,
			)
		: writable(untrack(() => defaultCollapsed));

	const isCollapsed = $derived($internalCollapsed);

	function setCollapsed(newValue: boolean) {
		if (isCollapsed === undefined) return;

		internalCollapsed.set(newValue);
		ontoggle?.(newValue);
	}

	function toggleCollapsed() {
		if (!collapsable || isCollapsed === undefined) return;

		setCollapsed(!isCollapsed);
	}

	export function open() {
		setCollapsed(false);
	}

	export function close() {
		setCollapsed(true);
	}

	export function toggle() {
		toggleCollapsed();
	}

	export function getIsCollapsed(): boolean {
		return isCollapsed ?? false;
	}
</script>

<div
	data-testid={testId}
	class="drawer"
	bind:this={containerDiv}
	bind:clientHeight
	class:collapsed={isCollapsed}
	class:bottom-border={bottomBorder && !isCollapsed}
	class:top-border={topBorder}
	class:highlighted
	class:grow
	class:noshrink
	class:rounded
	style:max-height={maxHeight}
>
	<DrawerHeader
		{actions}
		{closeActions}
		{transparent}
		sticky={stickyHeader}
		{reserveSpaceOnStuck}
		{scrollRoot}
		{onclose}
		ondblclick={toggleCollapsed}
		{closeButtonPlaceholder}
		{closeButtonPlaceholderWidth}
	>
		{#snippet content()}
			{#if collapsable}
				<button
					type="button"
					class="chevron-btn"
					class:expanded={!isCollapsed}
					onclick={(e) => {
						e.stopPropagation();
						toggleCollapsed();
					}}
				>
					<Icon name="chevron-right" />
				</button>
			{/if}

			{#if containerDiv}
				{@render header(containerDiv)}
			{/if}
		{/snippet}
	</DrawerHeader>

	{#snippet drawerContent()}
		<div class="drawer__content">
			{@render children()}
		</div>
	{/snippet}

	{#if !isCollapsed}
		{#if notScrollable}
			{@render drawerContent()}
		{:else}
			<AppScrollableContainer>
				{@render drawerContent()}
			</AppScrollableContainer>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.drawer {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-height: 100%;
		background-color: var(--bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--border-2);
		}
		&.top-border {
			border-top: 1px solid var(--border-2);
		}
		&.noshrink {
			flex-shrink: 0;
		}
		&.grow {
			flex-grow: 1;
		}
		&.collapsed {
			max-height: none;
		}

		&.rounded.collapsed {
			:global(.drawer-header) {
				border-bottom-color: var(--bg-2);
			}
		}

		&.highlighted {
			&::after {
				z-index: 1;
				position: absolute;
				top: 0;
				left: 0;
				width: 100%;
				height: 100%;
				border: 2px solid var(--fill-pop-bg);
				border-radius: calc(var(--radius-m) + 2px);
				content: "";
				pointer-events: none;
			}
		}
	}

	.drawer__content {
		container-name: drawer-content;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-direction: column;
	}

	.chevron-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--text-3);
		transition:
			color var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			color: var(--text-2);
		}

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.rounded {
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
	}
</style>

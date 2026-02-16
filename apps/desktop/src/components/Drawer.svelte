<script lang="ts">
	import ConfigurableScrollableContainer from "$components/ConfigurableScrollableContainer.svelte";
	import PreviewHeader from "$components/PreviewHeader.svelte";
	import Resizer from "$components/Resizer.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { inject } from "@gitbutler/core/context";
	import { persistWithExpiration } from "@gitbutler/shared/persisted";
	import { Icon } from "@gitbutler/ui";
	import { pxToRem } from "@gitbutler/ui/utils/pxToRem";
	import { writable, type Writable } from "svelte/store";
	import type { ComponentProps, Snippet } from "svelte";

	type Props = {
		header: Snippet<[HTMLDivElement]>;
		actions?: Snippet<[element: HTMLElement]>;
		children: Snippet;
		testId?: string;
		collapsable?: boolean;
		persistId?: string;
		topBorder?: boolean;
		bottomBorder?: boolean;
		grow?: boolean;
		noshrink?: boolean;
		clientHeight?: number;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		defaultCollapsed?: boolean;
		notScrollable?: boolean;
		maxHeight?: string;
		transparent?: boolean;
		stickyHeader?: boolean;
		rounded?: boolean;
		reserveSpaceOnStuck?: boolean;
		closeButtonPlaceholder?: boolean;
		scrollRoot?: HTMLElement | null;
		highlighted?: boolean;
		onclose?: () => void;
		ontoggle?: (collapsed: boolean) => void;
	};

	let {
		header,
		actions,
		children,
		testId,
		collapsable = true,
		persistId,
		bottomBorder = true,
		topBorder = false,
		grow,
		noshrink,
		resizer,
		clientHeight = $bindable(),
		defaultCollapsed = false,
		notScrollable = false,
		maxHeight,
		transparent,
		stickyHeader,
		rounded,
		reserveSpaceOnStuck,
		closeButtonPlaceholder,
		scrollRoot,
		highlighted,
		ontoggle,
		onclose,
	}: Props = $props();

	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let containerDiv = $state<HTMLDivElement>();
	let internalCollapsed: Writable<boolean | undefined> = persistId
		? persistWithExpiration<boolean>(defaultCollapsed, persistId, 1440)
		: writable(defaultCollapsed);

	const isCollapsed = $derived($internalCollapsed);

	let headerHeight = $state(0);
	let contentHeight = $state(0);
	const totalHeightRem = $derived(pxToRem(headerHeight + 1 + contentHeight, zoom));

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
	<PreviewHeader
		{actions}
		bind:headerHeight
		{transparent}
		sticky={stickyHeader}
		{reserveSpaceOnStuck}
		{scrollRoot}
		{onclose}
		ondblclick={toggleCollapsed}
		{closeButtonPlaceholder}
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
	</PreviewHeader>

	{#snippet drawerContent()}
		<div class="drawer__content" bind:clientHeight={contentHeight}>
			{@render children()}
		</div>
	{/snippet}

	{#if !isCollapsed}
		{#if notScrollable}
			{@render drawerContent()}
		{:else}
			<ConfigurableScrollableContainer>
				{@render drawerContent()}
			</ConfigurableScrollableContainer>
		{/if}
	{/if}

	{#if resizer}
		<!--
			This ternarny statement captures the nuance of maxHeight possibly
			being lower than minHeight.
			TODO: Move this logic into the resizer so it applies everywhere.
		-->
		{@const computedMaxHeight =
			resizer.maxHeight && resizer.minHeight
				? Math.min(resizer.maxHeight, Math.max(totalHeightRem, resizer.minHeight))
				: undefined}
		<Resizer
			viewport={containerDiv}
			defaultValue={undefined}
			passive={resizer.passive}
			disabled={isCollapsed}
			direction="down"
			{...resizer}
			maxHeight={computedMaxHeight}
		/>
	{/if}
</div>

<style lang="postcss">
	.drawer {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-height: 100%;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.top-border {
			border-top: 1px solid var(--clr-border-2);
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

		&.highlighted {
			&::after {
				z-index: 1;
				position: absolute;
				top: 0;
				left: 0;
				width: 100%;
				height: 100%;
				border: 2px solid var(--clr-theme-pop-element);
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
		color: var(--clr-text-3);
		transition:
			color var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			color: var(--clr-text-2);
		}

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.rounded {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>

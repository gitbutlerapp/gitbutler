<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Resizer from '$components/Resizer.svelte';
	import {
		INTELLIGENT_SCROLLING_SERVICE,
		scrollingAttachment,
		type TargetType
	} from '$lib/intelligentScrolling/service';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { writable, type Writable } from 'svelte/store';
	import type { ComponentProps, Snippet } from 'svelte';

	type Props = {
		title?: string;
		header?: Snippet<[HTMLDivElement]>;
		extraActions?: Snippet;
		kebabMenu?: Snippet<[element: HTMLElement]>;
		children?: Snippet;
		filesSplitView?: Snippet;
		testId?: string;
		persistId?: string;
		bottomBorder?: boolean;
		transparent?: boolean;
		scrollToId?: string;
		scrollToType?: TargetType;
		grow?: boolean;
		noshrink?: boolean;
		clientHeight?: number;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		onclose?: () => void;
		ontoggle?: (collapsed: boolean) => void;
	};

	let {
		title,
		header,
		extraActions,
		kebabMenu,
		children,
		filesSplitView,
		testId,
		persistId,
		bottomBorder,
		transparent,
		scrollToId,
		scrollToType,
		grow,
		noshrink,
		resizer,
		clientHeight = $bindable(),
		ontoggle,
		onclose
	}: Props = $props();

	const intelligentScrollingService = inject(INTELLIGENT_SCROLLING_SERVICE);
	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let headerDiv = $state<HTMLDivElement>();
	let containerDiv = $state<HTMLDivElement>();
	let collapsed: Writable<boolean | undefined> = $derived.by(() => {
		if (persistId) {
			return persistWithExpiration<boolean>(false, persistId, 1440);
		}
		return writable(false);
	});

	let headerHeight = $state(0);
	let contentHeight = $state(0);
	const totalHeightRem = $derived(pxToRem(headerHeight + 1 + contentHeight, zoom));
</script>

<div
	data-testid={testId}
	class="drawer"
	bind:this={containerDiv}
	bind:clientHeight
	class:collapsed={$collapsed}
	class:bottom-border={bottomBorder}
	class:transparent
	class:grow
	class:noshrink
	{@attach scrollingAttachment(intelligentScrollingService, scrollToId, scrollToType)}
>
	<div
		bind:this={headerDiv}
		class="drawer-header"
		class:bottom-border={!$collapsed}
		bind:clientHeight={headerHeight}
	>
		{#if $collapsed !== undefined}
			{@const name = $collapsed ? 'chevron-right' : ('chevron-down' as const)}
			<button
				type="button"
				class="chevron-btn focus-state"
				onclick={() => {
					if ($collapsed !== undefined) {
						const newValue = !$collapsed;
						collapsed.set(newValue);
						ontoggle?.(newValue);
					}
				}}
			>
				<Icon {name} />
			</button>
		{/if}

		<div class="drawer-header__title">
			{#if title}
				<h3 class="text-15 text-bold truncate">
					{title}
				</h3>
			{/if}
			{#if header}
				{@render header(containerDiv)}
			{/if}
		</div>

		{#if extraActions || kebabMenu || onclose}
			<div class="drawer-header__actions">
				{#if extraActions}
					{@render extraActions()}
				{/if}

				{#if kebabMenu}
					{@render kebabMenu(headerDiv)}
				{/if}

				{#if onclose}
					<Button kind="ghost" icon="cross" size="tag" onclick={() => onclose()} />
				{/if}
			</div>
		{/if}
	</div>

	{#if !$collapsed}
		<ConfigurableScrollableContainer>
			{#if children}
				<div class="drawer__content" bind:clientHeight={contentHeight}>
					{@render children()}
				</div>

				{#if filesSplitView}
					<div class="drawer__files-split-view">
						{@render filesSplitView()}
					</div>
				{/if}
			{/if}
		</ConfigurableScrollableContainer>
	{/if}
	{#if resizer}
		<!--
			This ternarny statement captures the nuance of maxHeight possibly
			being lower than minHeight.
			TODO: Move this logic into the resizer so it applies everwhere.
		-->
		{@const maxHeight =
			resizer.maxHeight && resizer.minHeight
				? Math.min(resizer.maxHeight, Math.max(totalHeightRem, resizer.minHeight))
				: undefined}
		<Resizer
			viewport={containerDiv}
			defaultValue={undefined}
			passive={resizer.passive}
			hidden={$collapsed}
			direction="down"
			imitateBorder
			{...resizer}
			{maxHeight}
		/>
	{/if}
</div>

<style>
	.drawer {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-height: 100%;
		overflow: hidden;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.transparent {
			background-color: transparent;
		}
		&.noshrink {
			flex-shrink: 0;
		}
		&.grow {
			flex-grow: 1;
		}
	}

	.drawer-header {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 12px 0 14px;
		gap: 8px;
		border-bottom: 1px solid transparent;
		background-color: var(--clr-bg-2);

		&.bottom-border {
			border-bottom-color: var(--clr-border-2);
		}
	}

	.drawer-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		height: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.drawer-header__actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		margin-right: -2px; /* buttons have some paddings that look not aligned. With this we "remove" them */
		gap: 4px;
	}

	.drawer__content {
		container-name: drawer-content;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex-direction: column;
	}

	.drawer__files-split-view {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	.chevron-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>

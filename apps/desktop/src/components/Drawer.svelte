<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import PreviewHeader from '$components/PreviewHeader.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { inject } from '@gitbutler/core/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import { Icon } from '@gitbutler/ui';

	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { writable, type Writable } from 'svelte/store';
	import type { ComponentProps, Snippet } from 'svelte';

	type Props = {
		header: Snippet<[HTMLDivElement]>;
		actions?: Snippet<[element: HTMLElement]>;
		children: Snippet;
		testId?: string;
		persistId?: string;
		bottomBorder?: boolean;
		grow?: boolean;
		noshrink?: boolean;
		clientHeight?: number;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		defaultCollapsed?: boolean;
		notScrollable?: boolean;
		onclose?: () => void;
		ontoggle?: (collapsed: boolean) => void;
	};

	let {
		header,
		actions,
		children,
		testId,
		persistId,
		bottomBorder,
		grow,
		noshrink,
		resizer,
		clientHeight = $bindable(),
		defaultCollapsed = false,
		notScrollable = false,
		ontoggle,
		onclose
	}: Props = $props();

	const userSettings = inject(SETTINGS);
	const zoom = $derived($userSettings.zoom);

	let containerDiv = $state<HTMLDivElement>();
	let collapsed: Writable<boolean | undefined> = persistId
		? persistWithExpiration<boolean>(defaultCollapsed, persistId, 1440)
		: writable(defaultCollapsed);

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
	class:bottom-border={bottomBorder && !resizer}
	class:grow
	class:noshrink
>
	<PreviewHeader {actions} {collapsed} bind:headerHeight {onclose}>
		{#snippet content()}
			<button
				type="button"
				class="chevron-btn focus-state"
				class:expanded={!$collapsed}
				onclick={(e) => {
					e.stopPropagation();
					if ($collapsed !== undefined) {
						const newValue = !$collapsed;
						collapsed.set(newValue);
						ontoggle?.(newValue);
					}
				}}
			>
				<Icon name="chevron-right" />
			</button>

			{#if containerDiv}
				{@render header(containerDiv)}
			{/if}
		{/snippet}
	</PreviewHeader>

	{#if !$collapsed}
		{#if notScrollable}
			<div class="drawer__content" bind:clientHeight={contentHeight}>
				{@render children()}
			</div>
		{:else}
			<ConfigurableScrollableContainer>
				<div class="drawer__content" bind:clientHeight={contentHeight}>
					{@render children()}
				</div>
			</ConfigurableScrollableContainer>
		{/if}
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
			disabled={$collapsed}
			direction="down"
			showBorder
			{...resizer}
			{maxHeight}
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
		overflow: hidden;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.noshrink {
			flex-shrink: 0;
		}
		&.grow {
			flex-grow: 1;
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
</style>

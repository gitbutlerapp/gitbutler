<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import {
		IntelligentScrollingService,
		scrollingAttachment,
		type TargetType
	} from '$lib/intelligentScrolling/service';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { writable, type Writable } from 'svelte/store';
	import type { Snippet } from 'svelte';

	type Props = {
		title?: string;
		header?: Snippet<[HTMLDivElement]>;
		extraActions?: Snippet;
		kebabMenu?: Snippet<[element: HTMLElement]>;
		children?: Snippet;
		filesSplitView?: Snippet;
		headerNoPaddingLeft?: boolean;
		testId?: string;
		persistId?: string;
		collapsible?: boolean;
		bottomBorder?: boolean;
		transparent?: boolean;
		scrollToId?: string;
		scrollToType?: TargetType;
		grow?: boolean;
		onclose?: () => void;
		resizer?: Snippet<[{ element: HTMLDivElement; collapsed?: boolean }]>;
	};

	const {
		title,
		header,
		extraActions,
		kebabMenu,
		children,
		filesSplitView,
		headerNoPaddingLeft,
		testId,
		persistId,
		collapsible,
		bottomBorder,
		transparent,
		scrollToId,
		scrollToType,
		grow,
		onclose,
		resizer
	}: Props = $props();

	const [intelligentScrollingService] = inject(IntelligentScrollingService);

	let headerDiv = $state<HTMLDivElement>();
	let containerDiv = $state<HTMLDivElement>();
	let collapsed: Writable<boolean | undefined> = $derived.by(() => {
		if (!collapsible) {
			return writable(undefined);
		}
		if (persistId) {
			return persistWithExpiration<boolean>(false, persistId, 1440);
		}
		return writable(false);
	});
</script>

<div
	data-testid={testId}
	class="drawer"
	bind:this={containerDiv}
	class:collapsed={$collapsed}
	class:bottom-border={bottomBorder}
	class:transparent
	class:grow
	class:no-shrink={resizer && $collapsed !== undefined}
	{@attach scrollingAttachment(intelligentScrollingService, scrollToId, scrollToType)}
>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		bind:this={headerDiv}
		class="drawer-header"
		class:no-padding-left={headerNoPaddingLeft}
		ondblclick={() => {
			if ($collapsed !== undefined) {
				collapsed.set(!$collapsed);
			}
		}}
	>
		{#if $collapsed !== undefined}
			{@const name = $collapsed ? 'chevron-down' : ('chevron-up' as const)}
			<div class="chevron">
				<Icon {name} />
			</div>
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

		<div class="drawer-header__actions">
			{#if extraActions}
				<div class="drawer-header__actions-group">
					{@render extraActions()}
				</div>
			{/if}
			<div class="drawer-header__actions-group">
				{#if kebabMenu}
					{@render kebabMenu(headerDiv)}
				{/if}

				{#if onclose}
					<Button kind="ghost" icon="cross" size="tag" onclick={() => onclose()} />
				{/if}
			</div>
		</div>
	</div>

	{#if !$collapsed}
		<ConfigurableScrollableContainer>
			{#if children}
				<div class="drawer__content">
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
	{@render resizer?.({ element: containerDiv, collapsed: $collapsed })}
</div>

<style>
	.drawer {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-height: calc(100% + 1px);
		overflow: hidden;
		background-color: var(--clr-bg-1);

		&.bottom-border {
			border-bottom: 1px solid var(--clr-border-2);
		}
		&.transparent {
			background-color: transparent;
		}
		&.no-shrink {
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
		padding: 0 8px 0 14px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);

		&.no-padding-left {
			padding-left: 0;
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
		gap: 12px;
	}

	.drawer-header__actions-group {
		display: flex;
		align-items: center;
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

	.chevron {
		display: flex;
		align-items: center;
		justify-content: center;
		padding-left: 14px;
	}
</style>

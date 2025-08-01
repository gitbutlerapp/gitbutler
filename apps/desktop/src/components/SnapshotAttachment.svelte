<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		foldable?: boolean;
		foldedAmount?: number | undefined;
		foldedHeight?: string;
		children?: Snippet;
	}

	const {
		foldable = false,
		foldedAmount = undefined,
		foldedHeight = '3rem',
		children
	}: Props = $props();

	let isOpen: boolean = $state(false);
	let el = $state<HTMLElement>();

	let contentHeight = $state<string>();

	function setHeight() {
		contentHeight = `calc(${pxToRem(el?.scrollHeight ?? 0)}rem + ${pxToRem(8)}rem)`;
	}

	onMount(() => {
		if (!foldable) return;

		setHeight();
	});

	$effect(() => {
		if (el) {
			setHeight();
		}
	});
</script>

<div class="snapshot-attachment">
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		bind:this={el}
		onclick={() => {
			if (foldable && !isOpen) {
				isOpen = true;
			}
		}}
		class="snapshot-attachment__content"
		style="max-height: {foldable ? (isOpen ? contentHeight : foldedHeight) : 'auto'}"
	>
		{@render children?.()}
	</div>
	{#if foldable}
		<button
			type="button"
			class="toggle-btn"
			onclick={() => {
				isOpen = !isOpen;
			}}
		>
			<span class="text-11">{isOpen ? 'Fold files' : `Show ${foldedAmount} files`}</span>
			<div class="toggle-btn__icon" style="transform: rotate({isOpen ? '180deg' : '0'})">
				<Icon name="chevron-down-small" />
			</div>
		</button>
	{/if}
</div>

<style lang="postcss">
	.snapshot-attachment {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.snapshot-attachment__content {
		display: flex;
		flex-direction: column;
		height: 100%;
		transition: max-height 0.2s ease;
	}

	.toggle-btn {
		display: flex;
		position: relative;
		align-items: center;
		justify-content: space-between;
		padding: 8px;
		border-top: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-2);
		transition:
			color var(--transition-fast),
			background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2);
			color: var(--clr-text-1);
		}
	}

	.toggle-btn__icon {
		display: flex;
		align-items: center;
	}
</style>

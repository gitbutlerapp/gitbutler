<script lang="ts">
	import Button from '$components/Button.svelte';
	import type { TooltipPosition, TooltipAlign } from '$components/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType, ComponentKindType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	type ButtonPropsSubset = {
		id?: string | undefined;
		el?: HTMLElement;
		// Interaction props
		disabled?: boolean;
		activated?: boolean;
		tabindex?: number | undefined;
		type?: 'submit' | 'reset' | 'button' | undefined;
		// Layout props
		shrinkable?: boolean;
		reversedDirection?: boolean;
		width?: number | undefined;
		maxWidth?: number | undefined;
		size?: 'tag' | 'button' | 'cta';
		wide?: boolean;
		grow?: boolean;
		align?: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto';
		dropdownChild?: boolean;
		// Style props
		style?: ComponentColorType;
		kind?: ComponentKindType;
		solidBackground?: boolean;
		// Additional elements
		icon?: keyof typeof iconsJson | undefined;
		tooltip?: string;
		tooltipPosition?: TooltipPosition;
		tooltipAlign?: TooltipAlign;
		tooltipDelay?: number;
		testId?: string;
		loading?: boolean;
		// Snippets
		children?: Snippet;
	};

	type Props = ButtonPropsSubset & { action: () => Promise<void>; stopPropagation?: boolean };
	let { action, loading = $bindable(), stopPropagation = false, ...rest }: Props = $props();

	let state = $state<'inert' | 'loading' | 'complete'>('inert');

	async function performAction(event: Event) {
		if (stopPropagation) {
			event.stopPropagation();
		}

		loading = true;

		try {
			await action();
		} finally {
			loading = false;
		}
	}

	const isLoading = $derived(loading ?? state === 'loading');
</script>

<Button onclick={performAction} loading={isLoading} {...rest}></Button>

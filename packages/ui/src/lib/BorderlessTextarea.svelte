<script lang="ts" module>
	export interface Props {
		ref?: HTMLTextAreaElement;
		value: string | undefined;
		placeholder?: string;
		readonly?: boolean;
		fontSize?: number;
		maxHeight?: string;
		rows?: number;
		autofocus?: boolean;
		padding?: {
			top: number;
			right: number;
			bottom: number;
			left: number;
		};
		oninput?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onfocus?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}
</script>

<script lang="ts">
	import { autoHeight } from '$lib/utils/autoHeight';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { onMount } from 'svelte';

	let {
		ref = $bindable(),
		value = $bindable(),
		placeholder,
		readonly,
		fontSize = 14,
		maxHeight = 'none',
		rows = 1,
		autofocus = false,
		padding = { top: 0, right: 0, bottom: 0, left: 0 },
		oninput,
		onfocus,
		onkeydown
	}: Props = $props();

	onMount(() => {
		setTimeout(() => {
			if (ref) {
				autoHeight(ref);
				if (autofocus) {
					ref.focus();
				}
			}
		}, 0);
	});

	$effect(() => {
		if (ref) {
			// reference the value to trigger
			// the effect when it changes
			// eslint-disable-next-line @typescript-eslint/no-unused-expressions
			value;
			autoHeight(ref);
		}
	});
</script>

<textarea
	tabindex="0"
	bind:this={ref}
	bind:value
	use:resizeObserver={(e) => {
		autoHeight(e.currentTarget as HTMLTextAreaElement);
	}}
	class="borderless-textarea scrollbar"
	{rows}
	{placeholder}
	{readonly}
	oninput={(e) => {
		oninput?.(e);
	}}
	onfocus={(e) => {
		autoHeight(e.currentTarget);
		onfocus?.(e);
	}}
	{onkeydown}
	style:font-size={pxToRem(fontSize)}
	style:max-height={maxHeight}
	style:padding-top={pxToRem(padding.top)}
	style:padding-right={pxToRem(padding.right)}
	style:padding-bottom={pxToRem(padding.bottom)}
	style:padding-left={pxToRem(padding.left)}
></textarea>

<style lang="postcss">
	.borderless-textarea {
		resize: none;
		outline: none;
		font-size: 14px;
		width: 100%;
		padding: 0;
		margin: 0;
		color: var(--clr-text-1);
		overflow-y: auto; /* Enable scrolling when max height is reached */
		background-color: transparent;
		/* background-color: rgba(0, 0, 0, 0.1); */
	}

	/* placeholder */
	::placeholder {
		color: var(--clr-text-3);
	}
</style>

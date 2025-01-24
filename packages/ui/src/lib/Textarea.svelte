<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { HTMLTextareaAttributes } from 'svelte/elements';

	interface Props extends HTMLTextareaAttributes {
		textBoxEl?: HTMLTextAreaElement;
		label?: string;
		value?: string;
		fontWeight?: 'regular' | 'bold' | 'semibold';
		fontSize?: number;
		minRows?: number;
		maxRows?: number;
		class?: string;
		flex?: string;
		padding?: {
			top: number;
			right: number;
			bottom: number;
			left: number;
		};
		borderless?: boolean;
		borderTop?: boolean;
		borderRight?: boolean;
		borderBottom?: boolean;
		borderLeft?: boolean;
		unstyled?: boolean;
	}

	let {
		id,
		textBoxEl = $bindable(),
		label,
		value = $bindable(),
		placeholder,
		disabled,
		fontSize = 13,
		minRows = 1,
		maxRows = 100,
		autofocus,
		class: className = '',
		fontWeight = 'regular',
		flex,
		padding = { top: 12, right: 12, bottom: 12, left: 12 },
		borderless,
		borderTop = true,
		borderRight = true,
		borderBottom = true,
		borderLeft = true,
		unstyled,
		oninput,
		onchange,
		onfocus,
		onblur,
		onkeydown
	}: Props = $props();

	let measureEl: HTMLPreElement | undefined = $state();

	$effect(() => {
		// mock textarea style
		if (textBoxEl && measureEl) {
			const textBoxElStyles = window.getComputedStyle(textBoxEl);

			measureEl.style.fontFamily = textBoxElStyles.fontFamily;
			measureEl.style.fontSize = textBoxElStyles.fontSize;
			measureEl.style.fontWeight = textBoxElStyles.fontWeight;
			measureEl.style.border = textBoxElStyles.border;
		}
	});

	$effect(() => {
		if (autofocus) {
			// set time out to ensure the element is rendered
			setTimeout(() => {
				textBoxEl?.focus();
			}, 0);
		}
	});

	const lineHeight = 1.6;

	const maxHeight = $derived(fontSize * maxRows + padding.top + padding.bottom);
	const minHeight = $derived(fontSize * minRows + padding.top + padding.bottom);

	let measureElHeight = $state(0);
</script>

<div
	class="textarea-container"
	style:--placeholder-text={`"${placeholder && placeholder !== '' ? placeholder : 'Type here...'}"`}
	style:--min-rows={minRows}
	style:--max-rows={maxRows}
	style:--padding-top={pxToRem(padding.top)}
	style:--padding-right={pxToRem(padding.right)}
	style:--padding-bottom={pxToRem(padding.bottom)}
	style:--padding-left={pxToRem(padding.left)}
	style:--lineheight-ratio={1.6}
	style:flex
>
	{#if label}
		<label class="textarea-label text-13 text-semibold text-body" for={id}>
			{label}
		</label>
	{/if}
	<pre
		class="textarea-measure-el"
		aria-hidden="true"
		bind:this={measureEl}
		bind:offsetHeight={measureElHeight}
		style:line-height={lineHeight}
		style:min-height={pxToRem(minHeight)}
		style:max-height={pxToRem(maxHeight)}>{value + '\n'}</pre>
	<textarea
		bind:this={textBoxEl}
		name={id}
		{id}
		class="textarea scrollbar {className} text-{fontWeight}"
		class:disabled
		class:text-input={!unstyled}
		class:textarea-unstyled={unstyled}
		class:hide-scrollbar={measureElHeight < maxHeight}
		style:height={pxToRem(measureElHeight)}
		style:font-size={pxToRem(fontSize)}
		style:border-top-width={borderTop && !borderless ? '1px' : '0'}
		style:border-right-width={borderRight && !borderless ? '1px' : '0'}
		style:border-bottom-width={borderBottom && !borderless ? '1px' : '0'}
		style:border-left-width={borderLeft && !borderless ? '1px' : '0'}
		style:border-top-right-radius={!borderTop || !borderRight ? '0' : undefined}
		style:border-top-left-radius={!borderTop || !borderLeft ? '0' : undefined}
		style:border-bottom-right-radius={!borderBottom || !borderRight ? '0' : undefined}
		style:border-bottom-left-radius={!borderBottom || !borderLeft ? '0' : undefined}
		{placeholder}
		bind:value
		{disabled}
		{oninput}
		{onchange}
		{onblur}
		{onkeydown}
		{onfocus}
		rows={minRows}
	></textarea>
</div>

<style lang="postcss">
	.textarea-container {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 6px;

		/* hide scrollbar */
		&::-webkit-scrollbar {
			display: none;
		}
	}

	@layer components {
		.textarea-unstyled {
			outline: none;
			border: none;
			background: transparent;
		}
	}

	.textarea-wrapper {
		position: relative;
		display: flex;
	}

	.textarea-measure-el,
	.textarea {
		padding: var(--padding-top) var(--padding-right) var(--padding-bottom) var(--padding-left);
		line-height: var(--lineheight-ratio);
		width: 100%;
		word-wrap: break-word;
		white-space: pre-wrap;
	}

	.textarea-measure-el {
		z-index: -1;
		position: absolute;
		background-color: rgba(0, 0, 0, 0.1);
		height: fit-content;
		margin: 0;
		pointer-events: none;
		overflow: hidden;
		visibility: hidden;
	}

	.textarea {
		font-family: var(--fontfamily-default);
		cursor: text;
		resize: none;
		overflow-y: auto; /* Enable scrolling when max height is reached */
		overflow-x: hidden;

		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

		&.hide-scrollbar {
			&::-webkit-scrollbar {
				display: none;
			}
		}

		&:disabled {
			cursor: default;
		}

		&::placeholder {
			color: var(--clr-text-3);
		}
	}

	.text-regular {
		font-weight: var(--text-weight-regular);
	}

	.textarea-label {
		color: var(--clr-text-2);
	}
</style>

<script lang="ts" module>
	export interface Props {
		value: string | undefined;
		placeholder?: string;
		disabled?: boolean;
		fontSize?: number;
		minRows?: number;
		maxRows?: number;
		autofocus?: boolean;
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
		oninput?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onfocus?: (
			this: void,
			e: FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement }
		) => void;
		onkeydown?: (e: KeyboardEvent) => void;
	}
</script>

<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	let {
		value = $bindable(),
		placeholder,
		disabled,
		fontSize = 13,
		minRows = 2,
		maxRows = 100,
		autofocus,
		padding = { top: 12, right: 12, bottom: 12, left: 12 },
		borderless,
		borderTop = true,
		borderRight = true,
		borderBottom = true,
		borderLeft = true,
		unstyled,
		oninput,
		onfocus,
		onkeydown
	}: Props = $props();

	let textBox: HTMLDivElement | undefined = $state();

	$effect(() => {
		if (autofocus) {
			textBox?.focus();
		}
	});

	$effect(() => {
		if (textBox) {
			if (!disabled) {
				textBox.setAttribute('contenteditable', 'true');
			} else {
				textBox.removeAttribute('contenteditable');
			}

			// if (value === '') {
			// 	textBox.classList.add('textarea-placeholder');
			// } else {
			// 	textBox.classList.remove('textarea-placeholder');
			// }
		}
	});
</script>

<div
	class="unstyled-textarea-container"
	style:--placeholder-text={`"${placeholder || placeholder !== '' ? placeholder : ' '}"`}
	style:--font-size={pxToRem(fontSize)}
	style:--min-rows={minRows}
	style:--max-rows={maxRows}
>
	<div
		bind:this={textBox}
		role="textbox"
		aria-multiline="true"
		tabindex={disabled ? -1 : 0}
		contenteditable
		bind:innerText={value}
		onfocus={(e: FocusEvent) => {
			if (e.currentTarget) {
				onfocus?.(e as FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement });
			}
		}}
		oninput={(e: Event) => {
			const innerText = (e.target as HTMLDivElement).innerText;
			const eventMock = { currentTarget: { value: innerText } } as Event & {
				currentTarget: EventTarget & HTMLTextAreaElement;
			};

			oninput?.(eventMock);
		}}
		onkeydown={(e) => {
			onkeydown?.(e);
		}}
		class:disabled
		class="borderless-textarea scrollbar"
		class:text-input={!unstyled}
		class:textarea-placeholder={value === ''}
		style:padding-top={pxToRem(padding.top)}
		style:padding-right={pxToRem(padding.right)}
		style:padding-bottom={pxToRem(padding.bottom)}
		style:padding-left={pxToRem(padding.left)}
		style:border-top-width={borderTop && !borderless && !unstyled ? '1px' : '0'}
		style:border-right-width={borderRight && !borderless && !unstyled ? '1px' : '0'}
		style:border-bottom-width={borderBottom && !borderless && !unstyled ? '1px' : '0'}
		style:border-left-width={borderLeft && !borderless && !unstyled ? '1px' : '0'}
		style:border-top-right-radius={!borderTop || !borderRight ? '0' : undefined}
		style:border-top-left-radius={!borderTop || !borderLeft ? '0' : undefined}
		style:border-bottom-right-radius={!borderBottom || !borderRight ? '0' : undefined}
		style:border-bottom-left-radius={!borderBottom || !borderLeft ? '0' : undefined}
	>
		{value}
	</div>
</div>

<style lang="postcss">
	.unstyled-textarea-container {
		display: contents;
	}

	.borderless-textarea {
		font-family: var(--base-font-family);
		line-height: var(--body-line-height);
		font-weight: var(--base-font-weight);
		cursor: text;

		resize: none;
		outline: none;
		width: 100%;
		padding: 0;
		margin: 0;
		font-size: var(--font-size);
		min-height: calc(var(--font-size) * 1.5 * var(--min-rows));
		max-height: calc(var(--font-size) * 1.5 * var(--max-rows));
		overflow-y: auto; /* Enable scrolling when max height is reached */

		&.disabled {
			cursor: default;
		}

		&.textarea-placeholder {
			display: block;
			white-space: pre-wrap;

			&:before {
				content: var(--placeholder-text);
				color: var(--clr-text-3);
				cursor: text;
				pointer-events: none;
				position: relative;
			}
		}
	}
</style>

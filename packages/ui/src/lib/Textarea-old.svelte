<script lang="ts" module>
	import { clickOutside } from './utils/clickOutside';
	export interface Props {
		id?: string;
		textBoxEl?: HTMLDivElement;
		label?: string;
		value?: string;
		placeholder?: string;
		disabled?: boolean;
		fontSize?: number;
		minRows?: number;
		maxRows?: number;
		autofocus?: boolean;
		spellcheck?: boolean;
		autoComplete?: string;
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
		oninput?: (e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
		onfocus?: (
			this: void,
			e: FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement }
		) => void;
		onblur?: (
			this: void,
			e: FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement }
		) => void;
		onkeydown?: (e: KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement }) => void;
	}
</script>

<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

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
		flex,
		padding = { top: 12, right: 12, bottom: 12, left: 12 },
		borderless,
		borderTop = true,
		borderRight = true,
		borderBottom = true,
		borderLeft = true,
		unstyled,
		oninput,
		onfocus,
		onblur,
		onkeydown
	}: Props = $props();

	let isEmpty = $state(value === '');

	function getSelectionRange() {
		const selection = window.getSelection();
		if (selection) {
			const range = selection.getRangeAt(0);
			return range;
		}
	}

	$effect(() => {
		if (autofocus) {
			textBoxEl?.focus();
		}
	});

	$effect(() => {
		if (textBoxEl) {
			if (!disabled) {
				textBoxEl.setAttribute('contenteditable', 'plaintext-only');
			} else {
				textBoxEl.removeAttribute('contenteditable');
			}
		}
	});

	$effect(() => {
		if (value === ' ' || value === '') {
			isEmpty = true;
		} else {
			isEmpty = false;
		}
	});
</script>

<div
	class="textarea-container"
	style:--placeholder-text={`"${placeholder && placeholder !== '' ? placeholder : 'Type here...'}"`}
	style:--font-size={pxToRem(fontSize)}
	style:--min-rows={minRows}
	style:--max-rows={maxRows}
	style:--padding-top={pxToRem(padding.top)}
	style:--padding-right={pxToRem(padding.right)}
	style:--padding-bottom={pxToRem(padding.bottom)}
	style:--padding-left={pxToRem(padding.left)}
	style:--line-height-ratio={1.6}
	style:flex
>
	{#if label}
		<label class="textarea-label text-13 text-semibold" for={id}>
			{label}
		</label>
	{/if}
	<div
		bind:this={textBoxEl}
		use:clickOutside={{ handler: () => textBoxEl?.blur() }}
		{id}
		role="textbox"
		aria-multiline="true"
		tabindex={disabled ? -1 : 0}
		contenteditable="plaintext-only"
		onfocus={(e: Event) => {
			if (e.currentTarget) {
				onfocus?.(e as FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement });
			}
		}}
		onblur={(e: Event) => {
			if (e.currentTarget) {
				onblur?.(e as FocusEvent & { currentTarget: EventTarget & HTMLTextAreaElement });
			}
		}}
		oninput={(e: Event) => {
			const innerText = (e.target as HTMLDivElement).innerText;
			const eventMock = { currentTarget: { value: innerText } } as Event & {
				currentTarget: EventTarget & HTMLTextAreaElement;
			};

			isEmpty = innerText === '';

			oninput?.(eventMock);
		}}
		onkeydown={(e: KeyboardEvent) => {
			const selection = getSelectionRange();

			const eventMock = {
				key: e.key,
				code: e.code,
				altKey: e.altKey,
				metaKey: e.metaKey,
				ctrlKey: e.ctrlKey,
				shiftKey: e.shiftKey,
				location: e.location,
				currentTarget: {
					value: (e.currentTarget as HTMLDivElement).innerText,
					selectionStart: selection?.startOffset,
					selectionEnd: selection?.endOffset
				}
			} as unknown as KeyboardEvent & { currentTarget: EventTarget & HTMLTextAreaElement };

			onkeydown?.(eventMock);
		}}
		class="textarea scrollbar {className}"
		class:disabled
		class:text-input={!unstyled}
		class:textarea-unstyled={unstyled}
		class:textarea-placeholder={isEmpty}
		style:border-top-width={borderTop && !borderless ? '1px' : '0'}
		style:border-right-width={borderRight && !borderless ? '1px' : '0'}
		style:border-bottom-width={borderBottom && !borderless ? '1px' : '0'}
		style:border-left-width={borderLeft && !borderless ? '1px' : '0'}
		style:border-top-right-radius={!borderTop || !borderRight ? '0' : undefined}
		style:border-top-left-radius={!borderTop || !borderLeft ? '0' : undefined}
		style:border-bottom-right-radius={!borderBottom || !borderRight ? '0' : undefined}
		style:border-bottom-left-radius={!borderBottom || !borderLeft ? '0' : undefined}
	>
		{value}
	</div>
</div>

<style lang="postcss">
	.textarea-container {
		display: flex;
		flex-direction: column;
		gap: 6px;
		overflow-x: hidden;
	}

	@layer components {
		.textarea-unstyled {
			outline: none;
			border: none;
		}
	}

	.textarea {
		font-family: var(--base-font-family);
		line-height: var(--body-line-height);
		font-weight: var(--base-font-weight);
		white-space: pre-wrap;
		cursor: text;
		resize: none;

		width: 100%;
		font-size: var(--font-size);
		min-height: calc(
			var(--font-size) * var(--line-height-ratio) * var(--min-rows) + var(--padding-top) +
				var(--padding-bottom)
		);
		max-height: calc(
			var(--font-size) * var(--line-height-ratio) * var(--max-rows) + var(--padding-top) +
				var(--padding-bottom)
		);
		padding: var(--padding-top) var(--padding-right) var(--padding-bottom) var(--padding-left);
		overflow-y: auto; /* Enable scrolling when max height is reached */
		overflow-x: hidden;
		word-wrap: break-word;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);

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
				position: absolute;
			}
		}
	}

	.textarea-label {
		color: var(--clr-text-2);
	}
</style>

<script lang="ts" module>
	type T = any | unknown | undefined;
</script>

<script lang="ts" generics="T extends undefined | any = any">
	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import { focusTrap } from '$lib/utils/focusTrap';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { onDestroy } from 'svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		width?: 'medium' | 'large' | 'small' | 'xsmall' | number;
		type?: 'info' | 'warning' | 'error' | 'success';
		title?: string;
		closeButton?: boolean;
		noPadding?: boolean;
		defaultItem?: T;
		/**
		 * Callback to be called when the modal is closed.
		 *
		 * Whether closing by clicking outside the modal, subtmitting the form or calling the `close` function.
		 * This is called after the closing animation is finished.
		 */
		onClose?: () => void;
		/**
		 * Callback to be called when the modal is closed by clicking outside the modal.
		 */
		onClickOutside?: () => void;
		onSubmit?: (close: () => void, item: T) => void;
		onKeyDown?: (e: KeyboardEvent) => void;
		children: Snippet<[item: T, close: () => void]>;
		controls?: Snippet<[close: () => void, item: T]>;
		testId?: string;
	};

	const {
		width = 'medium',
		title,
		type = 'info',
		closeButton,
		onClose,
		onClickOutside,
		children,
		controls,
		onSubmit,
		onKeyDown,
		noPadding = false,
		testId,
		defaultItem
	}: Props = $props();

	let open = $state(false);
	let item = $state<T>(defaultItem as any);
	let isClosing = $state(false);
	let closingPromise: Promise<void> | undefined = undefined;

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			close();
		}
	}

	// Clean up event listener if component is destroyed (in case modal is open)
	onDestroy(() => {
		window.removeEventListener('keydown', handleKeyDown);
	});

	export function show(newItem?: T) {
		item = newItem as any;
		open = true;

		window.addEventListener('keydown', handleKeyDown);
	}

	export function close(): Promise<void> {
		if (!open) return Promise.resolve();
		if (isClosing && closingPromise) return closingPromise;

		isClosing = true;
		closingPromise = new Promise((resolve) => {
			setTimeout(() => {
				item = undefined as any;
				open = false;
				isClosing = false;
				onClose?.();
				closingPromise = undefined;
				resolve();
			}, 100); // This should match the duration of the closing animation
		});

		return closingPromise;
	}

	export const imports = {
		get open() {
			return open;
		}
	};
</script>

{#if open}
	<div
		data-testid={testId}
		role="presentation"
		use:portal={'body'}
		class="modal-container {isClosing ? 'closing' : 'open'}"
		class:open
		onmousedown={(e) => {
			e.stopPropagation();

			if (e.target === e.currentTarget) {
				onClickOutside?.();
				close();
			}
		}}
		onkeydown={onKeyDown}
	>
		<form
			use:focusTrap
			class="modal-form"
			class:medium={width === 'medium'}
			class:large={width === 'large'}
			class:small={width === 'small'}
			class:xsmall={width === 'xsmall'}
			style:width={typeof width === 'number' ? pxToRem(width) : undefined}
			onsubmit={(e) => {
				e.preventDefault();
				onSubmit?.(close, item);
			}}
		>
			{#if title}
				<div class="modal__header">
					{#if type === 'warning'}
						<Icon name="warning" color="warning" />
					{/if}

					{#if type === 'error'}
						<Icon name="error" color="error" />
					{/if}

					{#if type === 'success'}
						<Icon name="success" color="success" />
					{/if}

					<h2 class="text-14 text-bold">
						{title}
					</h2>

					{#if closeButton}
						<div class="close-btn">
							<Button type="button" kind="ghost" icon="cross" onclick={close}></Button>
						</div>
					{/if}
				</div>
			{/if}

			<div class="modal__body text-13 text-body" class:no-padding={noPadding}>
				{#if children}
					{@render children(item, close)}
				{/if}
			</div>

			{#if controls}
				<div class="modal__footer">
					{@render controls(close, item)}
				</div>
			{/if}
		</form>
	</div>
{/if}

<style lang="postcss">
	.modal-container {
		z-index: var(--z-modal);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		padding: 24px;

		display: flex;
		justify-content: center;
		align-items: center;

		background-color: var(--clr-overlay-bg);
	}

	.modal-container.open {
		animation: dialog-fade-in 0.15s ease-out forwards;

		& .modal-form {
			animation: dialog-zoom-in 0.25s cubic-bezier(0.34, 1.35, 0.7, 1) forwards;
		}
	}

	.modal-container.closing {
		animation: dialog-fade-out 0.05s ease-out forwards;

		& .modal-form {
			animation: dialog-zoom-out 0.1s cubic-bezier(0.34, 1.35, 0.7, 1) forwards;
		}
	}

	.modal-form {
		display: flex;
		flex-direction: column;

		max-height: calc(100vh - 80px);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
	}

	.modal__header {
		position: relative;
		display: flex;
		align-items: center;
		padding: 16px;
		padding-bottom: 0;
		gap: 8px;
	}

	.close-btn {
		position: absolute;
		top: 10px;
		right: 10px;
	}

	.modal__body {
		display: flex;
		flex-direction: column;
		padding: 16px;
		line-height: 160%;
		overflow: hidden;

		&.no-padding {
			padding: 0;
		}
	}

	.modal__body :global(code),
	.modal__body :global(pre) {
		word-wrap: break-word;
	}

	.modal__footer {
		position: sticky;
		bottom: 0;
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		border-radius: 0 0 var(--radius-l) var(--radius-l);
	}

	/* ANIMATION */

	@keyframes dialog-zoom-in {
		from {
			transform: scale(0.95);
		}
		to {
			transform: scale(1);
		}
	}

	@keyframes dialog-zoom-out {
		from {
			transform: scale(1);
		}
		to {
			transform: scale(0.95);
		}
	}

	@keyframes dialog-fade-in {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	@keyframes dialog-fade-out {
		from {
			opacity: 1;
		}
		to {
			opacity: 0;
		}
	}

	/* MODIFIERS */

	.modal-form.medium {
		width: 580px;
	}

	.modal-form.large {
		width: 840px;
	}

	.modal-form.small {
		width: 380px;
	}

	.modal-form.xsmall {
		width: 310px;
	}
</style>

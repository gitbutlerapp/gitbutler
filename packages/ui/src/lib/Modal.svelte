<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { focusTrap } from '$lib/utils/focusTrap';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { onDestroy } from 'svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		width?: 'medium' | 'large' | 'small' | 'xsmall' | number;
		type?: 'info' | 'warning' | 'error' | 'success';
		title?: string;
		noPadding?: boolean;
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
		onSubmit?: (close: () => void) => void;
		onKeyDown?: (e: KeyboardEvent) => void;
		children: Snippet<[item: any, close: () => void]>;
		controls?: Snippet<[close: () => void, item: any]>;
	}

	const {
		width = 'medium',
		title,
		type = 'info',
		onClose,
		onClickOutside,
		children,
		controls,
		onSubmit,
		onKeyDown,
		noPadding = false
	}: Props = $props();

	let open = $state(false);
	let item = $state<any>();
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

	export function show(newItem?: any) {
		item = newItem;
		open = true;

		window.addEventListener('keydown', handleKeyDown);
	}

	export function close(): Promise<void> {
		if (!open) return Promise.resolve();
		if (isClosing && closingPromise) return closingPromise;

		isClosing = true;
		closingPromise = new Promise((resolve) => {
			setTimeout(() => {
				item = undefined;
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
		<div
			use:focusTrap
			class="modal-content"
			class:medium={width === 'medium'}
			class:large={width === 'large'}
			class:small={width === 'small'}
			class:xsmall={width === 'xsmall'}
			style:width={typeof width === 'number' ? pxToRem(width) : undefined}
		>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					onSubmit?.(close);
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

						<h2 class="text-14 text-semibold">
							{title}
						</h2>
					</div>
				{/if}

				<div class="modal__body custom-scrollbar text-13 text-body" class:no-padding={noPadding}>
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

		display: flex;
		justify-content: center;
		align-items: center;

		background-color: var(--clr-overlay-bg);
	}

	.modal-container.open {
		animation: dialog-fade-in 0.15s ease-out forwards;

		& .modal-content {
			animation: dialog-zoom-in 0.25s cubic-bezier(0.34, 1.35, 0.7, 1) forwards;
		}
	}

	.modal-container.closing {
		animation: dialog-fade-out 0.05s ease-out forwards;

		& .modal-content {
			animation: dialog-zoom-out 0.1s cubic-bezier(0.34, 1.35, 0.7, 1) forwards;
		}
	}

	.modal-content {
		display: flex;
		flex-direction: column;

		max-height: calc(100vh - 80px);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
		overflow: hidden;
	}

	.modal__header {
		display: flex;
		padding: 16px 16px 0;
		gap: 8px;
	}

	.modal__body {
		overflow: hidden;
		padding: 16px;
		line-height: 160%;

		&.no-padding {
			padding: 0;
		}
	}

	.modal__body :global(code),
	.modal__body :global(pre) {
		word-wrap: break-word;
	}

	.modal__footer {
		display: flex;
		width: 100%;
		justify-content: flex-end;
		gap: 8px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
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

	.modal-content.medium {
		width: 580px;
	}

	.modal-content.large {
		width: 840px;
	}

	.modal-content.small {
		width: 380px;
	}

	.modal-content.xsmall {
		width: 310px;
	}
</style>

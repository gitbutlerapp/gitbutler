<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { focusTrap } from '$lib/utils/focusTrap';
	import { portal } from '$lib/utils/portal';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { onDestroy } from 'svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		width?: 'medium' | 'large' | 'small' | 'xsmall' | number;
		title?: string;
		icon?: keyof typeof iconsJson;
		noPadding?: boolean;
		onClose?: () => void;
		onSubmit?: (close: () => void) => void;
		onKeyDown?: (e: KeyboardEvent) => void;
		children: Snippet<[item: any, close: () => void]>;
		controls?: Snippet<[close: () => void, item: any]>;
	}

	const {
		width = 'medium',
		title,
		icon,
		onClose,
		children,
		controls,
		onSubmit,
		onKeyDown,
		noPadding = false
	}: Props = $props();

	let open = $state(false);
	let item = $state<any>();
	let isClosing = $state(false);

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

	export function close() {
		isClosing = true;
		setTimeout(() => {
			item = undefined;
			open = false;
			isClosing = false;
			onClose?.();
		}, 100); // This should match the duration of the closing animation
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
		onclick={(e) => {
			console.log(e.target);
			e.stopPropagation();

			if (e.target === e.currentTarget) {
				close();
			}

			// close();
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
						{#if icon}
							<Icon name={icon} />
						{/if}
						<h2 class="text-14 text-semibold">
							{title}
						</h2>
					</div>
				{/if}

				<div class="modal__body custom-scrollbar text-13 text-body" class:no-padding={noPadding}>
					{@render children(item, close)}
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
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.modal__body {
		overflow: hidden;
		padding: 16px;
		line-height: 160%;

		&.no-padding {
			padding: 0;
		}
	}

	.modal__body > :global(code),
	.modal__body > :global(pre) {
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

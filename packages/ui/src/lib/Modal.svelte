<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import type iconsJson from '$lib/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		width?: 'default' | 'large' | 'small' | 'xsmall';
		title?: string;
		icon?: keyof typeof iconsJson;
		onclose?: () => void;
		onsubmit?: () => void;
		children: Snippet<[item?: any]>;
		controls?: Snippet<[close: () => void, item: any]>;
	}

	const { width = 'default', title, icon, onclose, children, controls, onsubmit }: Props = $props();

	let open = $state(false);
	let item = $state<any>();
	let dialogElement = $state<HTMLDialogElement>();

	export function show(newItem?: any) {
		item = newItem;
		open = true;
		dialogElement.showModal();
	}

	export function close() {
		item = undefined;
		open = false;
		onclose?.();
		dialogElement.close();
	}
</script>

<dialog
	bind:this={dialogElement}
	class="modal-content"
	class:default={width === 'default'}
	class:large={width === 'large'}
	class:small={width === 'small'}
	class:xsmall={width === 'xsmall'}
	class:round-top-corners={!title}
>
	{#if open}
		<form
			use:clickOutside={{
				handler: () => dialogElement.close()
			}}
			onsubmit={(e) => {
				e.preventDefault();
				onsubmit?.();
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

			<div class="modal__body custom-scrollbar text-13 text-body">
				{@render children(item)}
			</div>

			{#if controls}
				<div class="modal__footer">
					{@render controls(close, item)}
				</div>
			{/if}
		</form>
	{/if}
</dialog>

<style lang="postcss">
	dialog {
		outline: none;
		view-transition-name: dialog;
	}
	dialog[open] {
		animation: dialog-zoom 0.25s cubic-bezier(0.34, 1.35, 0.7, 1);
	}
	dialog.close::backdrop {
		animation: dialog-fade 0.15s ease-out;
	}

	dialog::backdrop {
		pointer-events: initial;
		background-color: var(--clr-overlay-bg);
	}
	dialog[open]::backdrop {
		animation: dialog-fade 0.15s ease-in;
	}

	.modal-content {
		display: flex;
		flex-direction: column;

		max-height: calc(100vh - 80px);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
	}

	.modal__header {
		display: flex;
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.modal__body {
		overflow: auto;
		padding: 16px;
		line-height: 160%;
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

	@keyframes dialog-zoom {
		from {
			transform: scale(0.95);
		}
		to {
			transform: scale(1);
		}
	}

	@keyframes dialog-fade {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	/* MODIFIERS */

	.modal-content.default {
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

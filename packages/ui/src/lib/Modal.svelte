<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import { clickOutside } from '$lib/utils/clickOutside';
	import { pxToRem } from '$lib/utils/pxToRem';
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
	let dialogElement = $state<HTMLDialogElement>();

	export function show(newItem?: any) {
		item = newItem;
		open = true;
		dialogElement?.showModal();
	}

	export function close() {
		item = undefined;
		open = false;
		onClose?.();
		dialogElement?.close();
	}

	export const imports = {
		get open() {
			return open;
		}
	};
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog
	tabindex="0"
	onkeydown={onKeyDown}
	bind:this={dialogElement}
	class:medium={width === 'medium'}
	class:large={width === 'large'}
	class:small={width === 'small'}
	class:xsmall={width === 'xsmall'}
	style:width={typeof width === 'number' ? pxToRem(width) : undefined}
>
	{#if open}
		<form
			use:clickOutside={{
				handler: close
			}}
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
	{/if}
</dialog>

<style lang="postcss">
	dialog {
		display: none;
		outline: none;
	}

	dialog[open] {
		display: flex;

		border: 1px solid var(--clr-border-2);

		flex-direction: column;

		max-height: calc(100vh - 80px);
		overflow: hidden;
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-l);

		/* fix for the native dialog "inherit" issue */
		text-align: left;

		animation: dialog-zoom 0.25s cubic-bezier(0.34, 1.35, 0.7, 1);

		/* MODIFIERS */
		&.medium {
			width: 580px;
		}

		&.large {
			width: 840px;
		}

		&.small {
			width: 380px;
		}

		&.xsmall {
			width: 310px;
		}
	}

	/* backdrop global */
	/* NOTE: temporarily hardcoded var(--clr-overlay-bg); */
	:global(dialog[open]::backdrop) {
		background-color: rgba(214, 214, 214, 0.4);
		animation: dialog-fade 0.15s ease-in;
	}

	/* backdrop dark */
	/* NOTE: temporarily hardcoded dark var(--clr-overlay-bg); */
	:global(html.dark dialog[open]::backdrop) {
		background-color: rgba(0, 0, 0, 0.35);
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
</style>

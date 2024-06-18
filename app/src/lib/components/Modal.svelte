<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import Icon from '$lib/components/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		width?: 'default' | 'small' | 'large';
		title?: string;
		icon?: keyof typeof iconsJson;
		onclose?: () => void;
		children: Snippet<[item?: any]>;
		controls?: Snippet<[close: () => void, item: any]>;
	}

	const { width = 'default', title, icon, onclose, children, controls }: Props = $props();

	let dialog = $state<HTMLDialogElement>();
	let item = $state<any>();
	let open = $state(false);

	export function show(newItem?: any) {
		item = newItem;
		dialog?.showModal();
		open = true;
	}

	export function close() {
		item = undefined;
		dialog?.close();
		open = false;
		onclose?.();
	}
</script>

<dialog
	class:s-default={width === 'default'}
	class:s-small={width === 'small'}
	class:s-large={width === 'large'}
	bind:this={dialog}
	onclose={close}
>
	{#if open}
		<div
			class="modal-content"
			use:clickOutside={{
				trigger: dialog,
				handler: () => dialog?.close()
			}}
		>
			{#if title}
				<div class="modal__header">
					{#if icon}
						<Icon name={icon} />
					{/if}
					<h2 class="text-base-14 text-semibold">
						{title}
					</h2>
				</div>
			{/if}

			<div class="modal__body custom-scrollbar">
				{@render children(item)}
			</div>

			{#if controls}
				<div class="modal__footer">
					{@render controls(close, item)}
				</div>
			{/if}
		</div>
	{/if}
</dialog>

<style lang="postcss">
	dialog[open] {
		display: flex;
		flex-direction: column;
		max-height: calc(100vh - 80px);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		box-shadow: var(--fx-shadow-l);
	}

	/* modifiers */

	.s-large {
		width: 840px;
	}

	.s-default {
		width: 580px;
	}

	.s-small {
		width: 380px;
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
</style>

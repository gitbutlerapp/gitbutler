<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Overlay from '../Overlay/Overlay.svelte';
	import { IconClose } from '$lib/icons';
	import type { ComponentType } from 'svelte';

	export function show(newItem?: any) {
		item = newItem;
		modal.show();
	}
	export function close() {
		item = undefined;
		modal.close();
	}

	export let width: 'default' | 'small' | 'large' = 'default';
	export let title: string | undefined = undefined;
	export let icon: ComponentType | undefined = undefined;

	let item: any;
	let modal: Overlay;
</script>

<Overlay bind:this={modal} let:close on:close>
	<div
		class="flex cursor-default flex-col gap-y-4 py-4"
		class:w-[680px]={width === 'default'}
		class:w-[380px]={width === 'small'}
		class:w-[980px]={width === 'large'}
	>
		{#if $$slots.title}
			<div
				class="flex w-full items-center justify-between border-b border-light-100 px-4 pb-4 pr-2 dark:border-dark-800"
			>
				<div class="flex items-center gap-2">
					<svelte:component this={icon} class="h-5 w-5" />
					<h2 class="text-lg font-medium">
						<slot name="title" />
					</h2>
				</div>
				<Button height="small" kind="plain" on:click={close} icon={IconClose} />
			</div>
		{/if}

		<!-- TODO: Remove this props based way once all other modals have been updated -->
		{#if title}
			<div
				class="flex w-full items-center justify-between border-b border-light-100 px-4 pb-4 pr-2 dark:border-dark-800"
			>
				<div class="flex items-center gap-2">
					<svelte:component this={icon} class="h-5 w-5" />

					<h2 class="text-lg">
						{title}
					</h2>
				</div>

				<Button height="small" kind="plain" on:click={close} icon={IconClose} />
			</div>
		{/if}

		<div class="flex max-h-[500px] flex-auto overflow-auto px-4">
			<slot {item} />
		</div>

		<div
			class="flex w-full justify-end gap-4 border-t border-light-100 px-4 pr-2 pt-4 dark:border-dark-800"
		>
			<slot name="controls" {item} {close}>
				<Button kind="outlined" on:click={close}>Secondary action</Button>
				<Button color="purple" on:click={close}>Primary action</Button>
			</slot>
		</div>
	</div>
</Overlay>

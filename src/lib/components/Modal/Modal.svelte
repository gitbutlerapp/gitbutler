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
		class="flex flex-col gap-y-4 p-4"
		class:w-[680px]={width === 'default'}
		class:w-[380px]={width === 'small'}
		class:w-[980px]={width === 'large'}
	>
		{#if title}
			<div class="flex w-full items-center justify-between">
				<div class="flex items-center gap-2">
					<svelte:component this={icon} class="h-5 w-5" />

					<h2 class="text-lg">
						{title}
					</h2>
				</div>

				<Button height="small" kind="plain" on:click={close} icon={IconClose} />
			</div>
		{/if}

		<main class="flex max-h-[500px] flex-auto overflow-auto">
			<slot {item} />
		</main>

		<div class="shadowk flex w-full justify-end gap-4">
			<slot name="controls" {item} {close}>
				<Button kind="outlined" on:click={close}>Secondary action</Button>
				<Button color="primary" on:click={close}>Primary action</Button>
			</slot>
		</div>
	</div>
</Overlay>

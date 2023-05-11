<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Overlay from '../Overlay/Overlay.svelte';
	import { IconClose } from '$lib/components/icons';
	import type { ComponentType } from 'svelte';

	export const show = () => modal.show();
	export const close = () => modal.close();

	export let width: 'default' | 'small' | 'large' = 'default';
	export let title: string | undefined = 'Title';
	export let icon: ComponentType | undefined = undefined;

	let modal: Overlay;
</script>

<Overlay bind:this={modal} let:close>
	<div
		class="modal"
		class:w-[680px]={width === 'default'}
		class:w-[380px]={width === 'small'}
		class:w-[980px]={width === 'large'}
	>
		{#if title}
			<header class="flex w-full items-center justify-between p-4 text-zinc-300">
				<div class="flex items-center gap-2">
					<svelte:component this={icon} class="h-5 w-5" />

					<h2 class="text-xl">
						{title}
					</h2>
				</div>

				<Button kind="plain" on:click={close} icon={IconClose} />
			</header>
		{/if}

		<main class="flex max-h-[500px] flex-auto overflow-auto p-4 text-text-default">
			<slot />
		</main>

		<footer class="shadowk flex w-full justify-end gap-4 p-4">
			<slot name="controls" {close}>
				<Button kind="outlined" on:click={close}>Secondary action</Button>
				<Button color="primary" on:click={close}>Primary action</Button>
			</slot>
		</footer>
	</div>
</Overlay>

<style>
	header {
		box-shadow: 0px -1px 0px 0px #0000001a inset;
	}

	main {
		border: 0.5px solid rgba(63, 63, 63, 0.5);
	}

	footer {
		box-shadow: 0px 1px 0px 0px #0000001a inset;
	}
</style>

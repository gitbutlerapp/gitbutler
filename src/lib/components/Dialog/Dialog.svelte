<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Modal from '../Modal.svelte';
	import { IconClose } from '$lib/components/icons';

	export const show = () => modal.show();

	let modal: Modal;
</script>

<Modal bind:this={modal} let:close>
	<div class="wrapper flex w-full flex-col text-zinc-300">
		<header class="flex w-full justify-between gap-4 p-4">
			<h2 class="text-xl ">
				<slot name="title">Title</slot>
			</h2>

			<Button filled={false} on:click={close} icon={IconClose} />
		</header>

		{#if $$slots.default}
			<div class="p-4 text-base ">
				<slot />
			</div>
		{/if}

		<footer class="flex w-full justify-end gap-4 p-4">
			<slot name="controls" {close}>
				<Button filled={false} outlined={true} on:click={close}>Secondary action</Button>
				<Button role="primary" on:click={close}>Primary action</Button>
			</slot>
		</footer>
	</div>
</Modal>

<style>
	.wrapper {
		background: linear-gradient(0deg, rgba(43, 43, 48, 0.8), rgba(43, 43, 48, 0.8)),
			linear-gradient(0deg, rgba(63, 63, 63, 0.5), rgba(63, 63, 63, 0.5));
	}

	header {
		box-shadow: inset 0px -1px 0px rgba(0, 0, 0, 0.1);
	}

	footer {
		box-shadow: inset 0px 1px 0px rgba(0, 0, 0, 0.1);
	}
</style>

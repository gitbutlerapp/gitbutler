<script lang="ts">
	import Button from '../Button/Button.svelte';
	import Modal from '../Modal.svelte';
	import { IconClose } from '$lib/components/icons';

	export const show = () => modal.show();
	export const close = () => modal.close();

	let modal: Modal;
</script>

<Modal bind:this={modal} let:close>
	<div class="modal modal-delete-project flex w-full flex-col text-zinc-300">
		<header class="flex w-full justify-between gap-4 p-4">
			<h2 class="text-xl ">
				<slot name="title">Title</slot>
			</h2>

			<Button kind="plain" on:click={close} icon={IconClose} />
		</header>

		{#if $$slots.default}
			<div class="p-4 text-base ">
				<slot />
			</div>
		{/if}

		<footer class="flex w-full justify-end gap-4 p-4">
			<slot name="controls" {close}>
				<Button kind="outlined" on:click={close}>Secondary action</Button>
				<Button color="primary" on:click={close}>Primary action</Button>
			</slot>
		</footer>
	</div>
</Modal>

<style>
	header {
		box-shadow: inset 0px -1px 0px rgba(0, 0, 0, 0.1);
	}

	footer {
		box-shadow: inset 0px 1px 0px rgba(0, 0, 0, 0.1);
	}
</style>

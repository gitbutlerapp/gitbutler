<script lang="ts">
	import Modal from './Modal.svelte';
	import Button from '../Button/Button.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	let modal: Modal;

	let title = 'Title';
</script>

<Hst.Story title="Modal">
	<Button on:click={modal.show}>Open Modal</Button>

	<Hst.Variant title="No content">
		<Modal bind:this={modal}>
			<svelte:fragment slot="title">
				{title}
			</svelte:fragment>
		</Modal>
	</Hst.Variant>

	<Hst.Variant title="With content">
		<Modal bind:this={modal}>
			<svelte:fragment slot="title">
				{title}
			</svelte:fragment>
			<p>Some content</p>
		</Modal>
	</Hst.Variant>
</Hst.Story>

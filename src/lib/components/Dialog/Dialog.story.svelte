<script lang="ts">
	import Dialog from './Dialog.svelte';
	import Button from '../Button/Button.svelte';

	export let Hst: import('@histoire/plugin-svelte').Hst;

	let dialog: Dialog;

	let title = 'Title';
</script>

<Hst.Story title="Dialog">
	<Button on:click={dialog.show}>Open Dialog</Button>

	<Hst.Variant title="No content">
		<Dialog bind:this={dialog}>
			<svelte:fragment slot="title">
				{title}
			</svelte:fragment>
		</Dialog>
	</Hst.Variant>

	<Hst.Variant title="With content">
		<Dialog bind:this={dialog}>
			<svelte:fragment slot="title">
				{title}
			</svelte:fragment>
			<p>Some content</p>
		</Dialog>
	</Hst.Variant>
</Hst.Story>

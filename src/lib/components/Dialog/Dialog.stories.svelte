<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Dialog from './Dialog.svelte';
	import Button from '../Button/Button.svelte';

	let dialog: Dialog;
	let count = 0;
</script>

<Meta title="GitButler/Dialog" component={Dialog} />

<Story name="Dialog with title only">
	<Button on:click={() => dialog.show()}>Open Dialog</Button>
	<Dialog bind:this={dialog} />
</Story>

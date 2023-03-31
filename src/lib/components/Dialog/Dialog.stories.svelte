<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Dialog from './Dialog.svelte';
	import Button from '../Button/Button.svelte';

	let dialog: Dialog;
	let count = 0;
</script>

<Meta title="GitButler/Dialog" component={Dialog} />

<Story name="Dialog with title only">
	<Button label="Open Dialog" on:click={() => dialog.show()} />
	<Dialog
		title="Dialog title"
		secondaryActionLabel="Cancel"
		primaryActionLabel="You clicked: {count}"
		primaryAction={() => count++}
		bind:this={dialog}
	/>
</Story>

<Story name="Dialog with title and body">
	<Button label="Open Dialog" on:click={() => dialog.show()} />
	<Dialog
		title="Dialog title"
		content="Dialog body content"
		secondaryActionLabel="Cancel"
		primaryActionLabel="You clicked: {count}"
		primaryAction={() => count++}
		bind:this={dialog}
	/>
</Story>

<Story name="Dialog small">
	<Button label="Open Dialog" on:click={() => dialog.show()} />
	<Dialog
		title="Dialog title"
		content="Dialog body content"
		size="small"
		secondaryActionLabel="Cancel"
		primaryActionLabel="You clicked: {count}"
		primaryAction={() => count++}
		bind:this={dialog}
	/>
</Story>

<Story name="Dialog large">
	<Button label="Open Dialog" on:click={() => dialog.show()} />
	<Dialog
		title="Dialog title"
		content="Dialog body content"
		size="large"
		secondaryActionLabel="Cancel"
		primaryActionLabel="You clicked: {count}"
		primaryAction={() => count++}
		bind:this={dialog}
	/>
</Story>

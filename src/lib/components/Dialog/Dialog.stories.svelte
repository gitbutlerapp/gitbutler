<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';

	import Dialog from './Dialog.svelte';
	import Button from '../Button/Button.svelte';

	let dialog: Dialog;
</script>

<Meta title="GitButler/Dialog" component={Dialog} />

<Story name="Dialog with title only">
	<Button on:click={() => dialog.show()}>Open Dialog</Button>
	<Dialog bind:this={dialog} />
</Story>

<Story name="Dialog with content">
	<Button on:click={() => dialog.show()}>Open Dialog</Button>
	<Dialog bind:this={dialog}>
		<p class="w-[346px]">
			GitButler offers support for generating Git commits automatically. To use this feature, you
			will need to sync with GitButler cloud.
		</p>
	</Dialog>
</Story>

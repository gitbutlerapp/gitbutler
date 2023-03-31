<script lang="ts">
	import { Button, Dialog } from '$lib/components';

	export let title: string;
	export let content: string | undefined = undefined;
	export let secondaryActionLabel = 'Cancel';
	export let primaryActionLabel = 'Cancel';
	export let size = 'default';

	let dialog: Dialog;
</script>

<!--
@component
This exists only to facilitate testing of the `Dialog` component in Storybook.
Do not use this in the app.
Do not style this.
Do not add any logic to this.
-->
<div>
	<Button label="Open Dialog" on:click={() => dialog.show()} />
	<Dialog
		{title}
		{content}
		{secondaryActionLabel}
		{primaryActionLabel}
		{size}
		primaryAction={() => console.log('primary action')}
		bind:this={dialog}
	/>
</div>

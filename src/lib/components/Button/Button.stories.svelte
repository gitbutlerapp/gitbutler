<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';
	import { IconHome } from '../icons';

	import Button from './Button.svelte';

	let count = 0;
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic">
	<Button on:click={() => count++} label="You clicked: {count}" />
</Story>

<Story name="Basic With Icon">
	<Button on:click={() => count++} label="You clicked: {count}">
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
	</Button>
</Story>

<Story name="Small With Icon">
	<Button on:click={() => count++} label="You clicked: {count}" small>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
	</Button>
</Story>

<Story name="Wide With Icon">
	<Button on:click={() => count++} label="You clicked: {count}" wide>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
	</Button>
</Story>


<Story name="Basic Outlined">
	<Button on:click={() => count++} label="You clicked: {count}" outlined />
</Story>

<Story name="Basic Small">
	<Button on:click={() => count++} label="You clicked: {count}" small />
</Story>

<Story name="Basic Outlined Small">
	<Button on:click={() => count++} label="You clicked: {count}" small outlined />
</Story>

<Story name="Primary">
	<Button on:click={() => count++} label="You clicked: {count}" primary />
</Story>

<Story name="Primary Small">
	<Button on:click={() => count++} label="You clicked: {count}" primary small />
</Story>

<Story name="Primary Wide">
	<Button on:click={() => count++} label="You clicked: {count}" primary wide />
</Story>

<Story name="Primary Small Wide">
	<Button on:click={() => count++} label="You clicked: {count}" primary wide small />
</Story>

<Story name="Primary Outlined Small">
	<Button on:click={() => count++} label="You clicked: {count}" primary outlined small />
</Story>

<Story name="Link button">
	<Button href="https://gitbutler.com" label="GitButler" />
</Story>

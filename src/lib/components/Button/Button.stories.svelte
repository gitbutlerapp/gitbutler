<script lang="ts">
	import { Meta, Story } from '@storybook/addon-svelte-csf';
	import { IconHome } from '../icons';

	import Button from './Button.svelte';

	let count = 0;
</script>

<Meta title="GitButler/Button" component={Button} />

<Story name="Basic">
	<Button on:click={() => count++}>
		You clicked: {count}
	</Button>
</Story>

<Story name="Basic With Icon">
	<Button on:click={() => count++}>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
		You clicked: {count}
	</Button>
</Story>

<Story name="Small With Icon">
	<Button on:click={() => count++} small>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
		You clicked: {count}
	</Button>
</Story>

<Story name="Outlined icon">
	<Button on:click={() => count++} outlined>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
	</Button>
</Story>

<Story name="Wide With Icon">
	<Button on:click={() => count++} wide>
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
		You clicked: {count}
	</Button>
</Story>

<Story name="Basic Outlined">
	<Button on:click={() => count++} outlined>
		You clicked: {count}
	</Button>
</Story>

<Story name="Basic Small">
	<Button on:click={() => count++} small>
		You clicked: {count}
	</Button>
</Story>

<Story name="Basic Outlined Small">
	<Button on:click={() => count++} small outlined>
		You clicked: {count}
	</Button>
</Story>

<Story name="Primary">
	<Button on:click={() => count++} primary>
		You clicked: {count}
	</Button>
</Story>

<Story name="Primary Small">
	<Button on:click={() => count++} primary small>
		You clicked: {count}
	</Button>
</Story>

<Story name="Primary Wide">
	<Button on:click={() => count++} primary wide>
		You clicked: {count}
	</Button>
</Story>

<Story name="Primary Small Wide">
	<Button on:click={() => count++} primary wide small>
		You clicked: {count}
	</Button>
</Story>

<Story name="Primary Outlined Small">
	<Button on:click={() => count++} primary outlined small>
		You clicked: {count}
	</Button>
</Story>

<Story name="Link">
	<Button href="https://gitbutler.com">Go to https://gitbutler.com</Button>
</Story>

<Story name="Link with icon">
	<Button href="https://gitbutler.com">
		<svelte:fragment slot="icon">
			<IconHome />
		</svelte:fragment>
		Go to https://gitbutler.com
	</Button>
</Story>

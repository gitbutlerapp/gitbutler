<script lang="ts">
	import Button from '$lib/inputs/Button.svelte';
	import type { ComponentProps } from 'svelte';

	type Props = ComponentProps<Button> & { contents: string };

	const { contents, ...args }: Props = $props();
</script>

<Button {...args}>
	{contents}
</Button>

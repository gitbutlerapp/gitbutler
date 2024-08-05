<script lang="ts">
	import Button, { type ButtonProps } from '$lib/inputs/Button.svelte';

	const props: ButtonProps = $props();
</script>

<Button {...props}>Button</Button>

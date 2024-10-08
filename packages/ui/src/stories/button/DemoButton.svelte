<script lang="ts">
	import Button, { type Props as ButtonProps } from '$lib/Button.svelte';

	const props: ButtonProps = $props();
</script>

<Button {...props}>Button</Button>

<script>
	import { goto } from '$app/navigation';
	import Button from './Button.svelte';
</script>

<Button
	kind="outlined"
	color="neutral"
	on:click={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
>
	<slot />
</Button>

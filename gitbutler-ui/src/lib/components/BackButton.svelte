<script>
	import Button from './Button.svelte';
	import { goto } from '$app/navigation';
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

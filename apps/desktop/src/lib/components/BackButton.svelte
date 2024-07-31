<script>
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { goto } from '$app/navigation';
</script>

<Button
	style="ghost"
	outline
	on:mousedown={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
>
	<slot />
</Button>

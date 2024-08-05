<script>
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { goto } from '$app/navigation';
</script>

<Button
	style="ghost"
	outline
	onmousedown={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
>
	<slot />
</Button>

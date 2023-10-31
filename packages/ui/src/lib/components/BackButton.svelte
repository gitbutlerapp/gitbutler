<script>
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { goto } from '$app/navigation';
</script>

<button
	on:click={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
>
	<IconChevronLeft />
</button>

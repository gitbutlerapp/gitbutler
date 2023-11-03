<script>
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { goto } from '$app/navigation';

	let className = '';
	export { className as class };
</script>

<button
	class={className}
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

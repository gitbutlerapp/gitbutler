<script>
	import { onMount } from 'svelte';

	export let value;

	let textarea;

	onMount(() => {
		textarea.select();
		document.execCommand('copy');
	});
</script>

<textarea bind:value bind:this={textarea} />

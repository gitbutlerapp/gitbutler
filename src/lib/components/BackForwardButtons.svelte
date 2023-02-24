<script>
	import FaArrowLeft from 'svelte-icons/fa/FaArrowLeft.svelte';
	import FaArrowRight from 'svelte-icons/fa/FaArrowRight.svelte';
	let history = window.history;
</script>

<div class="flex items-center justify-center space-x-3 text-zinc-400">
	<button class="w-4 h-4  hover:text-zinc-200" title="Go back" on:click={() => history.back()}
		><FaArrowLeft /></button
	>
	<button class="w-4 h-4 hover:text-zinc-200" title="Go forward" on:click={() => history.forward()}
		><FaArrowRight /></button
	>
</div>

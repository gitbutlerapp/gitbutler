<script lang="ts">
	export let disabled = false;
	export let title = '';
</script>

<button
	on:click|preventDefault
	{disabled}
	{title}
	class="w-full px-3 py-0.5 text-left no-underline enabled:hover:bg-light-100 disabled:text-light-600 enabled:dark:hover:bg-dark-800"
>
	<slot />
</button>

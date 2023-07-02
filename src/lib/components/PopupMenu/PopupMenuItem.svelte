<script lang="ts">
	export let disabled = false;
</script>

<button
	on:click|preventDefault
	{disabled}
	class="w-full px-4 py-2 text-sm no-underline enabled:hover:bg-light-100 disabled:text-light-600 enabled:dark:hover:bg-dark-600"
>
	<slot />
</button>

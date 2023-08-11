<script lang="ts">
	export let name = '';

	export let large = false;
	export let disabled = false;
	export let checked = false;
	export let error = false;
	export let indeterminate = false;
	export let value = '';
</script>

<input
	on:click|stopPropagation
	on:change
	type="checkbox"
	{value}
	{name}
	class:large
	class:error
	{disabled}
	bind:checked
	bind:indeterminate
	class="cursor-pointer"
/>

<style>
</style>

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
	on:click
	on:change
	type="checkbox"
	{value}
	{name}
	class:large
	class:error
	{disabled}
	bind:checked
	bind:indeterminate
/>

<style>
	input {
		appearance: none;
		border: 0;
		border-radius: 4px;
		display: inline-grid;
		place-content: center;
	}

	input[type='checkbox']::before {
		content: '';
		width: 10px;
		height: 8px;
		transform: scale(0);
		box-shadow: inset 1em 1em white;
	}

	input[type='checkbox']:checked::before {
		transform: scale(1);
		clip-path: polygon(14% 44%, 0 65%, 50% 100%, 100% 16%, 80% 0%, 43% 62%);
		transform-origin: bottom left;
	}

	input[type='checkbox']:indeterminate::before {
		transform: scale(1);
		clip-path: inset(30% 7% 30% 7%);
	}

	/* colors */
	input {
		background: linear-gradient(180deg, #505050 0%, #656565 100%);
	}
	input.error {
		background: #fff4f4;
		border: 2px solid #d82c0d;
	}

	input:hover {
		background: linear-gradient(0deg, rgba(0, 0, 0, 0.2), rgba(0, 0, 0, 0.2)),
			linear-gradient(180deg, #505050 0%, #656565 100%);
	}

	input:checked,
	input:indeterminate {
		background: linear-gradient(180deg, #6b66aa 0%, #484383 100%);
	}
	input.error:checked,
	input.error:indeterminate {
		background: #d82c0d;
	}

	input:focus {
		background: #ffffff;
		border: 2px solid #58529f;
		border-radius: 5px;
	}
	input:checked:focus,
	input:indeterminate:focus {
		border: 2px solid #458fff;
		background: linear-gradient(180deg, #6b66aa 0%, #484383 100%);
		border-radius: 5px;
	}

	input:disabled {
		background: #fafbfb;
		border: 2px solid #d2d5d8;
	}
	input:checked:disabled,
	input:indeterminate:disabled {
		background: #bdc1cc;
	}

	/* sizes */
	input {
		width: 16px;
		height: 16px;
	}
	input.large {
		width: 20px;
		height: 20px;
	}
</style>

<script lang="ts">
	export let name = '';

	export let small = false;
	export let disabled = false;
	export let checked = false;
	export let indeterminate = false;
	export let value = '';
</script>

<input
	on:click|stopPropagation
	on:change
	type="checkbox"
	class="checkbox"
	class:small
	{value}
	{name}
	{disabled}
	bind:checked
	bind:indeterminate
/>

<style lang="postcss">
	.checkbox {
		appearance: none;
		width: var(--size-btn-xxs);
		height: var(--size-btn-xxs);
		border-radius: var(--radius-s);
		background-color: var(--clr-theme-container-light);
		/* background-color: red; */
		border: 1px solid var(--clr-theme-container-outline-light);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* not checked */
		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-pale);
			border-color: var(--clr-theme-container-outline-pale);
			outline: none;

			&::after {
				opacity: 0.2;
				transform: scale(0.8);
			}
		}

		&:focus {
			border-color: var(--clr-theme-container-outline-sub);
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-theme-scale-ntrl-60);
			border-color: none;
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);
			border-color: var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-dim);
				border-color: var(--clr-theme-pop-element-dim);
			}

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				opacity: 1;
				filter: invert(0);
				transform: scale(1);
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			width: 100%;
			height: 100%;
			border-radius: var(--radius-s);
			background-image: url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAiIGhlaWdodD0iNyIgdmlld0JveD0iMCAwIDEwIDciIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+CjxwYXRoIGQ9Ik05IDEuNUw0LjkyMTM5IDUuNzQ4NTVDNC41Mjc4MyA2LjE1ODUxIDMuODcyMTcgNi4xNTg1MSAzLjQ3ODYxIDUuNzQ4NTZMMSAzLjE2NjY3IiBzdHJva2U9IndoaXRlIiBzdHJva2Utd2lkdGg9IjEuNSIvPgo8L3N2Zz4K');
			background-position: center;
			background-size: 80%;
			background-repeat: no-repeat;
			transition: background-color var(--transition-fast);
			filter: invert(1);
			transform: scale(0.4);
			opacity: 0;
		}
	}
</style>

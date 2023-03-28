<script lang="ts">
	export let primary = false;
	export let filled = true;
	export let small = false;
	export let label: string;
</script>

<!--
@component
This is the only button we should be using in the app. 
It emits a click event like any self respecting button should.

It takes the following required props:
- `label` - string - the text to display on the button

And the following optional props:
- `primary` - boolean - whether the button should be primary or not
- `filled` - boolean - whether the button should be filled or not
- `small` - boolean - whether the button should be small or not

- Usage:
  ```tsx
<Button label="OK" on:click={yourFunction}/>
  ```
-->

<button
	class="
    base
    {primary
		? filled
			? 'primary-filled'
			: 'primary-nofill'
		: filled
		? 'default-filled'
		: 'default-nofill'}
    {small ? 'size-small' : 'size-normal'}
        "
	type="button"
	on:click
>
	{label}
</button>

<style>
	.base {
		@apply flex items-center justify-center rounded text-base text-white ;
	}
	.base:disabled {
		@apply opacity-40;
	}

	/* Primary */
	.primary-filled {
		border: 1px solid #3662e3;
		background: #3662e3;
	}
	.primary-filled:hover {
		border: 1px solid #1C48C9;
		background: #1C48C9;
	}
	.primary-nofill {
		background: rgba(28, 72, 201, 0.0);
		border: 1px solid #3662e3;
	}
	.primary-nofill:hover {
		background: rgba(28, 72, 201, 0.3);
		border: 1px solid #3662e3;
	}

	/* Default */
	.default-filled {
		border: 1px solid #71717a;
		background: #71717a;
	}
	.default-filled:hover {
		@apply border-zinc-600 bg-zinc-600;
	}
	.default-nofill {
		background: rgba(113, 113, 122, 0.0);
		border: 1px solid #71717a;
	}
	.default-nofill:hover {
		background: rgba(113, 113, 122, 0.4);
		border: 1px solid #71717a;
	}

	/* Size */
	.size-normal {
		@apply px-4 py-2;
	}
	.size-small {
		@apply px-2 py-1;
	}

</style>

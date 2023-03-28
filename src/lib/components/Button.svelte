<script lang="ts">
	export let primary = false;
	export let filled = true;
	export let small = false;
	export let wide = false;
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
    {small ? 'height-small' : 'height-normal'}
	{wide ? 'width-normal' : 'width-small'}
        "
	type="button"
	on:click
>
	{label}
</button>

<style>
	.base {
		@apply flex items-center justify-center rounded text-base text-zinc-50 shadow transition ease-in-out;
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
		border: 1px solid #1c48c9;
		background: #1c48c9;
		@apply transition ease-in-out;
	}
	.primary-nofill {
		background: rgba(28, 72, 201, 0);
		border: 1px solid #3662e3;
		@apply transition ease-in-out;
	}
	.primary-nofill:hover {
		background: rgba(28, 72, 201, 0.3);
		border: 1px solid #3662e3;
		@apply transition ease-in-out;
	}

	/* Default */
	.default-filled {
		border: 1px solid #71717a;
		background: #71717a;
		@apply transition ease-in-out;
	}
	.default-filled:hover {
		@apply border-zinc-600 bg-zinc-600;
	}
	.default-nofill {
		background: rgba(113, 113, 122, 0);
		border: 1px solid #71717a;
		@apply transition ease-in-out;
	}
	.default-nofill:hover {
		background: rgba(113, 113, 122, 0.4);
		border: 1px solid #71717a;
		@apply transition ease-in-out;
	}

	/* Size */
	.height-normal {
		@apply py-2;
	}
	.height-small {
		@apply py-1;
	}
	.width-normal {
		@apply px-[42.75px];
	}
	.width-small {
		@apply px-[16px];
	}
</style>

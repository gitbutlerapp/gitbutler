<script lang="ts">
	export let primary = false;
	export let outlined = true;
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
- `outlined` - boolean - whether the button should be outlined or not
- `small` - boolean - whether the button should be small or not

- Usage:
  ```tsx
<Button label="Label" on:click={yourFunction}/>
  ```
-->

<button
	class="
    btn-base
    {primary
		? outlined
			? 'btn-primary-outline'
			: 'btn-primary'
		: outlined
		? 'btn-basic-outline'
		: 'btn-basic'}
    {small ? 'btn-height-small' : 'btn-height-normal'}
	{wide ? 'btn-width-normal' : 'btn-width-small'}
        "
	type="button"
	on:click
>
	{label}
</button>

<style>
	.btn-base {
		@apply flex items-center justify-center rounded text-base text-zinc-50 shadow transition ease-in-out;
		border-top: 1px solid rgba(255, 255, 255, 0.2);
		border-bottom: 1px solid rgba(0, 0, 0, 0.3);
		border-left: 1px solid rgba(255, 255, 255, 0);
		border-right: 1px solid rgba(255, 255, 255, 0);
		text-shadow: 0px 2px #00000021;
	}
	.btn-base:disabled {
		@apply opacity-40;
	}

	/* Primary */
	.btn-primary {
		background: #3662e3;
	}
	.btn-primary:hover {
		background: #1c48c9;
		@apply transition ease-in-out;
	}
	.btn-primary-outline {
		background: rgba(28, 72, 201, 0);
		border: 1px solid #3662e3;
		@apply transition ease-in-out;
	}
	.btn-primary-outline:hover {
		background: rgba(28, 72, 201, 0.3);
		border: 1px solid #3662e3;
		@apply transition ease-in-out;
	}

	/* Basic */
	.btn-basic {
		background: #71717a;
		@apply transition ease-in-out;
	}
	.btn-basic:hover {
		@apply border-zinc-600 bg-zinc-600;
	}
	.btn-basic-outline {
		background: rgba(113, 113, 122, 0);
		border: 1px solid #71717a;
		@apply transition ease-in-out;
	}
	.btn-basic-outline:hover {
		background: rgba(113, 113, 122, 0.4);
		border: 1px solid #71717a;
		@apply transition ease-in-out;
	}

	/* Size */
	.btn-height-normal {
		@apply py-2;
	}
	.btn-height-small {
		@apply py-1;
	}
	.btn-width-normal {
		@apply px-[42.75px];
	}
	.btn-width-small {
		@apply px-[16px];
	}
</style>

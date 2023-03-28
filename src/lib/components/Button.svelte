<script lang="ts">
	export let primary = false;
	export let filled = false;
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
	<span class="overflow-hidden text-ellipsis">{label}</span>
</button>

<style>
	.base {
		@apply flex items-center justify-center rounded text-base text-white;
	}
	.base:hover {
		/* @apply bg-gray-100; */
	}
	.base:disabled {
		@apply opacity-40;
	}
	.primary-nofill {
		border: 1px solid #3662e3;
	}
	.primary-filled {
		background: #3662e3;
	}
	.default-nofill {
		background: #71717a;
	}
	.default-filled {
		border: 1px solid #71717a;
	}
	.size-normal {
		width: 66px;
		height: 36px;
	}
	.size-small {
		width: 66px;
		height: 24px;
	}
</style>

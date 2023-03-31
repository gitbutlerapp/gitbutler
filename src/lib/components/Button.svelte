<script lang="ts">
	export let primary = false;
	export let outlined = false;
	export let disabled = false;
	export let small = false;
	export let wide = false;
	export let label: string;
	export let type: 'button' | 'submit' = 'button';
	export let href: string | undefined = undefined;
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
- `href` - string - if this is set, the button will be a link instead of a button
- `type` - string - the type of button, defaults to `button`
- `disabled` - boolean - whether the button is disabled or not

- Usage:
  ```tsx
<Button label="Label" on:click={yourFunction}/>
  ```
-->

{#if href}
	<a
		{href}
		class="btn-base"
		class:btn-disabled={disabled}
		class:btn-primary-outline={primary && outlined}
		class:btn-primary={primary && !outlined}
		class:btn-basic-outline={!primary && outlined}
		class:btn-basic={!primary && !outlined}
		class:btn-height-small={small}
		class:btn-height-normal={!small}
		class:btn-width-normal={wide}
		class:btn-width-small={!wide}
	>
		{label}
	</a>
{:else}
	<button
		{type}
		on:click
		class="btn-base"
		class:btn-disabled={disabled}
		class:btn-primary-outline={primary && outlined}
		class:btn-primary={primary && !outlined}
		class:btn-basic-outline={!primary && outlined}
		class:btn-basic={!primary && !outlined}
		class:btn-height-small={small}
		class:btn-height-normal={!small}
		class:btn-width-normal={wide}
		class:btn-width-small={!wide}
	>
		{label}
	</button>
{/if}

<style lang="postcss">
	.btn-base {
		@apply flex items-center justify-center rounded text-base text-zinc-50 shadow transition ease-in-out;
		border-top: 1px solid rgba(255, 255, 255, 0.2);
		border-bottom: 1px solid rgba(0, 0, 0, 0.3);
		border-left: 1px solid rgba(255, 255, 255, 0);
		border-right: 1px solid rgba(255, 255, 255, 0);
		text-shadow: 0px 2px #00000021;
		white-space: nowrap;
	}

	.btn-disabled {
		@apply opacity-40;
		pointer-events: none;
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

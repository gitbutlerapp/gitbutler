<script module lang="ts">
	import Button from '$lib/Button.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { ComponentColor } from '$lib/utils/colorTypes';

	const { Story } = defineMeta({
		title: 'Inputs / Button',
		args: {
			label: 'Button'
		}
	});

	const defaultProps = {
		label: 'Button',
		reversedDirection: false
	};
</script>

{#snippet buttons({
	label,
	outline,
	style,
	reversedDirection
}: {
	label: string;
	outline: boolean;
	style?: ComponentColor;
	reversedDirection?: boolean;
})}
	<div class="group">
		<Button size="cta" {style} kind="soft" icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="cta" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="cta" {style} kind="soft" {reversedDirection}>{label}</Button>
		<Button size="cta" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="cta" {style} kind="soft" icon="plus-small" />
		<Button size="cta" {style} kind="solid" {outline} icon="plus-small" />
	</div>
	<div class="group">
		<Button size="button" {style} kind="soft" icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="button" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="button" {style} kind="soft" {reversedDirection}>{label}</Button>
		<Button size="button" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="button" {style} kind="soft" icon="plus-small" />
		<Button size="button" {style} kind="solid" {outline} icon="plus-small" />
	</div>
	<div class="group">
		<Button size="tag" {style} kind="soft" icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="tag" {style} kind="soft" {reversedDirection}>{label}</Button>
		<Button size="tag" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="tag" {style} kind="soft" icon="plus-small" />
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small" />
	</div>
{/snippet}

<Story name="All">
	<div class="wrapper">
		{@render buttons({
			label: defaultProps.label,
			outline: true,
			style: 'ghost',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'neutral',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'pop',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'success',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'error',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'warning',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			outline: false,
			style: 'purple',
			reversedDirection: defaultProps.reversedDirection
		})}
	</div></Story
>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.group {
		display: flex;
		gap: 8px;
	}
	.divider {
		display: block;
		width: 100%;
		height: 1px;
		background-color: #858585;
		opacity: 0.3;
		margin: 8px 0;
		border: none;
	}
</style>

<script module lang="ts">
	import Button from '$components/Button.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import type { ComponentColorType } from '$lib/utils/colorTypes';

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

{#snippet buttonGroup({
	size,
	label,
	style,
	reversedDirection
}: {
	size: 'icon' | 'cta' | 'button' | 'tag';
	label: string;
	style?: ComponentColorType;
	reversedDirection?: boolean;
})}
	<div class="group">
		<Button {size} {style} kind="solid" icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button {size} {style} kind="outline" icon="plus-small" {reversedDirection}>{label}</Button>
		<Button {size} {style} kind="ghost" icon="plus-small" {reversedDirection}>{label}</Button>

		<Button {size} {style} kind="solid" {reversedDirection}>{label}</Button>
		<Button {size} {style} kind="outline" {reversedDirection}>{label}</Button>
		<Button {size} {style} kind="ghost" {reversedDirection}>{label}</Button>

		<Button {size} {style} kind="solid" {reversedDirection} hotkey="Esc">{label}</Button>
		<Button {size} {style} kind="outline" {reversedDirection} hotkey="Esc">{label}</Button>
		<Button {size} {style} kind="ghost" {reversedDirection} hotkey="Esc">{label}</Button>

		<Button {size} {style} kind="solid" icon="plus-small" />
		<Button {size} {style} kind="outline" icon="plus-small" />
		<Button {size} {style} kind="ghost" icon="plus-small" />
	</div>
{/snippet}

{#snippet buttons({
	label,
	style,
	reversedDirection
}: {
	label: string;
	style?: ComponentColorType;
	reversedDirection?: boolean;
})}
	{@render buttonGroup({
		size: 'cta',
		label,
		style,
		reversedDirection
	})}
	{@render buttonGroup({
		size: 'button',
		label,
		style,
		reversedDirection
	})}
	{@render buttonGroup({
		size: 'tag',
		label,
		style,
		reversedDirection
	})}
	{@render buttonGroup({
		size: 'icon',
		label,
		style,
		reversedDirection
	})}
{/snippet}

<Story name="All">
	<div class="wrapper">
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			style: 'gray',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			style: 'pop',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			style: 'safe',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			style: 'danger',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
			style: 'warning',
			reversedDirection: defaultProps.reversedDirection
		})}
		<hr class="divider" />
		{@render buttons({
			label: defaultProps.label,
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
		margin: 8px 0;
		border: none;
		background-color: #858585;
		opacity: 0.3;
	}
</style>

<script lang="ts">
	import Button from '$lib/Button.svelte';
	import type { ComponentColor } from '$lib/utils/colorTypes';

	interface Props {
		label: string;
		reversedDirection?: boolean;
	}

	const { label, reversedDirection }: Props = $props();
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
		<Button size="cta" {style} icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="cta" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="cta" {style} {reversedDirection}>{label}</Button>
		<Button size="cta" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="cta" {style} icon="plus-small" />
		<Button size="cta" {style} kind="solid" {outline} icon="plus-small" />
	</div>
	<div class="group">
		<Button size="button" {style} icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="button" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="button" {style} {reversedDirection}>{label}</Button>
		<Button size="button" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="button" {style} icon="plus-small" />
		<Button size="button" {style} kind="solid" {outline} icon="plus-small" />
	</div>
	<div class="group">
		<Button size="tag" {style} icon="plus-small" {reversedDirection}>{label}</Button>
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small" {reversedDirection}>
			{label}
		</Button>
		<Button size="tag" {style} {reversedDirection}>{label}</Button>
		<Button size="tag" {style} kind="solid" {outline} {reversedDirection}>{label}</Button>
		<Button size="tag" {style} icon="plus-small" />
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small" />
	</div>
{/snippet}

<div class="wrapper">
	{@render buttons({ label, outline: true, style: 'ghost', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'neutral', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'pop', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'success', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'error', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'warning', reversedDirection })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'purple', reversedDirection })}
</div>

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

<script lang="ts">
	import Button from '../../lib/Button.svelte';

	interface Props {
		label: string;
		outline: boolean;
	}

	const { label }: Props = $props();
</script>

{#snippet buttons({ label, outline, style })}
	<div class="group">
		<Button size="cta" {style} icon="plus">{label}</Button>
		<Button size="cta" {style} kind="solid" {outline} icon="plus">{label}</Button>
		<Button size="cta" {style}>{label}</Button>
		<Button size="cta" {style} kind="solid" {outline}>{label}</Button>
		<Button size="cta" {style} icon="plus" />
		<Button size="cta" {style} kind="solid" {outline} icon="plus" />
	</div>
	<div class="group">
		<Button size="button" {style} icon="plus">{label}</Button>
		<Button size="button" {style} kind="solid" {outline} icon="plus">{label}</Button>
		<Button size="button" {style}>{label}</Button>
		<Button size="button" {style} kind="solid" {outline}>{label}</Button>
		<Button size="button" {style} icon="plus" />
		<Button size="button" {style} kind="solid" {outline} icon="plus" />
	</div>
	<div class="group">
		<Button size="tag" {style} icon="plus-small">{label}</Button>
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small">{label}</Button>
		<Button size="tag" {style}>{label}</Button>
		<Button size="tag" {style} kind="solid" {outline}>{label}</Button>
		<Button size="tag" {style} icon="plus-small" />
		<Button size="tag" {style} kind="solid" {outline} icon="plus-small" />
	</div>
{/snippet}

<div class="wrapper">
	{@render buttons({ label, outline: true, style: 'ghost' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'neutral' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'pop' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'success' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'error' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'warning' })}
	<hr class="divider" />
	{@render buttons({ label, outline: true, style: 'purple' })}
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
		background-color: #e0e0e0;
		margin: 8px 0;
		border: none;
	}
</style>

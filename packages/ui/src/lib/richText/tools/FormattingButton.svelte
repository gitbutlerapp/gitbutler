<script lang="ts">
	import Button from '$components/Button.svelte';
	import type iconsJson from '$lib/data/icons.json';

	interface Props {
		icon: keyof typeof iconsJson;
		activated?: boolean;
		tooltip: string;
		onclick: (e: MouseEvent) => void;
		disabled?: boolean;
		loading?: boolean;
		size?: 'tag' | 'button';
	}

	const { icon, activated, tooltip, onclick, disabled, loading, size = 'button' }: Props = $props();
</script>

<div class="formatting-button" class:formatting-button--activated={activated}>
	<Button
		type="button"
		style="gray"
		kind="ghost"
		{size}
		{icon}
		{activated}
		{disabled}
		{onclick}
		{tooltip}
		tooltipDelay={500}
		tooltipPosition="bottom"
		{loading}
	/>
</div>

<style lang="postcss">
	.formatting-button {
		display: flex;
		position: relative;

		&.formatting-button--activated {
			&::after {
				position: absolute;
				top: 0;
				right: 0;
				width: 6px;
				height: 6px;
				border-radius: 50%;
				background-color: var(--clr-theme-pop-element);
				content: '';
			}
		}
	}
</style>

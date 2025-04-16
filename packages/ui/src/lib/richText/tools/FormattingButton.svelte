<script lang="ts">
	import Button from '$lib/Button.svelte';
	import type iconsJson from '$lib/data/icons.json';

	interface Props {
		icon: keyof typeof iconsJson;
		activated?: boolean;
		tooltip: string;
		onclick: (e: MouseEvent) => void;
		disabled?: boolean;
		loading?: boolean;
	}

	const { icon, activated, tooltip, onclick, disabled, loading }: Props = $props();
</script>

<div class="formatting-button" class:formatting-button--activated={activated}>
	<Button
		type="button"
		style="neutral"
		kind="ghost"
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
		position: relative;
		display: flex;

		&.formatting-button--activated {
			&::after {
				content: '';
				position: absolute;
				top: 0;
				right: 0;
				width: 6px;
				height: 6px;
				background-color: var(--clr-theme-pop-element);
				border-radius: 50%;
			}
		}
	}
</style>

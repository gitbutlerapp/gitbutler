<script lang="ts">
	import Button from '$lib/Button.svelte';
	import type iconsJson from '$lib/data/icons.json';

	interface Props {
		icon: keyof typeof iconsJson;
		activated?: boolean;
		tooltip: string;
		onclick: (e: MouseEvent) => void;
		disabled?: boolean;
	}

	const { icon, activated, tooltip, onclick, disabled }: Props = $props();
</script>

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
/>

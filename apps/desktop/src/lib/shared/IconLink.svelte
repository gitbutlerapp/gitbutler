<script lang="ts">
	export let href: string;
	import Icon from '$lib/shared/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let iconOpacity: number = 0.7;
	export let icon: keyof typeof iconsJson;
</script>

<a class="link" target="_blank" {href}>
	<Icon name={icon} opacity={iconOpacity} />
	<span class="text-base-12"><slot /></span>
</a>

<style lang="postcss">
	.link {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 4px 6px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: oklch(from var(--clr-scale-ntrl-0) l c h / 0.05);
		}
	}
</style>

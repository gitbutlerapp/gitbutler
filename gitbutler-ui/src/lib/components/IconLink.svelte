<script lang="ts">
	export let href: string;
	import Icon from '$lib/components/Icon.svelte';
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
		gap: var(--size-10);
		padding: var(--size-4) var(--size-6);
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-0), transparent 95%);
		}
	}
</style>

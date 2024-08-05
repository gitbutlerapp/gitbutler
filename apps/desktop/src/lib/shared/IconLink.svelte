<script lang="ts">
	export let href: string;
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type iconsJson from '@gitbutler/ui/icons.json';

	export let icon: keyof typeof iconsJson;
</script>

<a class="link" target="_blank" {href}>
	<Icon name={icon} />
	<span class="text-base-12"><slot /></span>
</a>

<style lang="postcss">
	.link {
		display: flex;
		align-items: center;
		width: fit-content;
		gap: 10px;
		padding: 4px 6px;
		border-radius: var(--radius-m);

		color: var(--clr-scale-ntrl-40);
		text-decoration: none;

		transition: background-color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			background-color: oklch(from var(--clr-scale-ntrl-0) l c h / 0.05);
		}
	}
</style>

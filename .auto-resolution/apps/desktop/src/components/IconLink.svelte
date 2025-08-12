<script lang="ts">
	import { openExternalUrl } from '$lib/utils/url';

	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface Props {
		href: string;
		icon: keyof typeof iconsJson;
		children: Snippet;
	}

	const { href, icon, children }: Props = $props();
</script>

<button type="button" class="link" onclick={async () => await openExternalUrl(href)}>
	<Icon name={icon} />
	<span class="text-12">
		{@render children()}
	</span>
</button>

<style lang="postcss">
	.link {
		display: flex;
		align-items: center;
		width: fit-content;
		padding: 4px 6px;
		gap: 10px;
		border-radius: var(--radius-m);

		color: var(--clr-scale-ntrl-40);
		text-decoration: none;

		transition: background-color var(--transition-fast);

		&:hover {
			background-color: oklch(from var(--clr-scale-ntrl-0) l c h / 0.05);
			color: var(--clr-text-1);
		}
	}
</style>

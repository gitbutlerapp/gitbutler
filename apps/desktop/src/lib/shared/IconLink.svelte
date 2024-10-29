<script lang="ts">
	import { openExternalUrl } from '$lib/utils/url';
	import Icon from '@gitbutler/ui/Icon.svelte';
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
		gap: 10px;
		padding: 4px 6px;
		border-radius: var(--radius-m);

		color: var(--clr-scale-ntrl-40);
		text-decoration: none;

		transition: background-color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			background-color: color-mix(in srgb, var(--clr-scale-ntrl-0), transparent 50%);
		}
	}
</style>

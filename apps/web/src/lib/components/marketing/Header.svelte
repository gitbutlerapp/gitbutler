<script lang="ts">
	import MobileMenu from '$home/components/MobileMenu.svelte';
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		disableLogoLink?: boolean;
	}

	const { disableLogoLink }: Props = $props();
</script>

<!-- Link snippet for reusable navigation links -->
{#snippet link(props: { href: string; label: string; icon?: keyof typeof iconsJson })}
	<a
		href={props.href}
		target="_self"
		rel="noopener noreferrer"
		class="text-14 text-semibold link-snippet"
		data-sveltekit-preload-data="hover"
	>
		<span>{props.label}</span>
		{#if props.icon}
			<Icon name={props.icon} />
		{/if}
	</a>
{/snippet}

<header class="header">
	<GitbutlerLogoLink disabled={disableLogoLink} />

	<nav class="header-nav">
		<section class="flex gap-20">
			{@render link({
				href: jsonLinks.resources.documentation.url,
				label: jsonLinks.resources.documentation.label
			})}
			{@render link({
				href: jsonLinks.resources.source.url,
				label: 'Source',
				icon: 'github-outline'
			})}
			{@render link({
				href: jsonLinks.social.discord.url,
				label: 'Community',
				icon: 'discord-outline'
			})}
			{@render link({
				href: jsonLinks.resources.blog.url,
				label: jsonLinks.resources.blog.label
			})}
			{@render link({
				href: jsonLinks.resources.downloads.url,
				label: jsonLinks.resources.downloads.label
			})}
			{@render link({
				href: jsonLinks.resources.jobs.url,
				label: jsonLinks.resources.jobs.label
			})}
			<HeaderAuthSection />
		</section>
	</nav>

	<MobileMenu />
</header>

<style lang="postcss">
	.header {
		display: flex;
		grid-column: narrow-start / off-center;
		align-items: center;
		justify-content: space-between;
		padding-top: 40px;

		@media (--tablet-viewport) {
			padding-top: 32px;
		}

		@media (--mobile-viewport) {
			padding-top: 16px;
		}
	}

	.header-nav {
		display: flex;
		align-items: center;
		gap: 32px;

		@media (--tablet-viewport) {
			display: none;
		}
	}

	.link-snippet {
		display: flex;
		align-items: center;
		gap: 6px;
		text-decoration: none;
		text-decoration-color: var(--clr-theme-pop-element);
		text-decoration-thickness: 2px;
		text-underline-offset: 4px;
		transition:
			color var(--transition-fast),
			text-decoration var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			text-decoration-line: underline;
		}
	}
</style>

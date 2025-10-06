<script lang="ts">
	import MobileMenu from '$home/components/MobileMenu.svelte';
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import { Icon } from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
</script>

<!-- Link snippet for reusable navigation links -->
{#snippet link(props: {
	href: string;
	label: string;
	icon?: keyof typeof iconsJson;
	target?: string;
	rel?: string;
})}
	<a
		href={props.href}
		target={props.target ?? '_blank'}
		rel={props.rel ?? 'noopener noreferrer'}
		class="text-14 text-semibold link-snippet"
	>
		<span>{props.label}</span>
		{#if props.icon}
			<Icon name={props.icon} />
		{/if}
	</a>
{/snippet}

<header class="header">
	<GitbutlerLogoLink />

	<nav class="header-nav">
		<section class="flex gap-20">
			{@render link({
				href: jsonLinks.resources.downloads.url,
				label: jsonLinks.resources.downloads.label,
				target: '_self',
				rel: ''
			})}
			{@render link({
				href: jsonLinks.resources.documentation.url,
				label: jsonLinks.resources.documentation.label
			})}
			{@render link({
				href: jsonLinks.resources.source.url,
				label: 'View Source',
				icon: 'github-outline'
			})}
			{@render link({
				href: jsonLinks.social.discord.url,
				label: 'Community',
				icon: 'discord-outline'
			})}
			{@render link({
				href: jsonLinks.resources.jobs.url,
				label: jsonLinks.resources.jobs.label
			})}
		</section>
	</nav>

	<MobileMenu />
</header>

<style lang="scss">
	.header {
		display: flex;
		grid-column: narrow-start / off-gridded;
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

	.social-dropdown {
		display: inline-block;
		position: relative;
	}

	.social-button {
		display: flex;
		align-items: center;
		border: none;
		background: none;
		color: inherit;
		cursor: pointer;

		&:hover .social-button-label {
			text-decoration: underline dotted;
		}
	}

	.social-button-arrow {
		margin-left: 4px;
		font-size: 10px;
		transition: transform var(--transition-fast);

		&.rotated {
			transform: rotate(180deg);
		}
	}

	.social-link-arrow {
		display: inline-block;
		transform: scale(0.7);
		opacity: 0;
		transition: transform 0.15s ease-in-out;
	}

	.social-link {
		display: block;
		padding: 8px 14px;
		border-radius: var(--radius-s);
		text-decoration: none;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2);
			text-decoration: none;

			.social-link-arrow {
				transform: scale(1) translate(2px, -2px);
				opacity: 0.7;
			}
		}
	}

	.dropdown-menu {
		z-index: 1000;
		position: absolute;
		top: 100%;
		right: 0;
		min-width: 160px;
		margin-top: 8px;
		padding: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: 6px;
		background: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-m);
	}
</style>

<script lang="ts">
	import GitbutlerLogoLink from '$lib/components/GitbutlerLogoLink.svelte';
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';
	import * as jsonLinks from '$lib/data/links.json';
	import { fly } from 'svelte/transition';

	const socialLinks = Object.values(jsonLinks.social);

	let isDropdownOpen = false;

	function toggleDropdown() {
		isDropdownOpen = !isDropdownOpen;
	}

	function closeDropdown() {
		isDropdownOpen = false;
	}

	function clickOutside(node: HTMLElement) {
		function handleClick(event: MouseEvent) {
			if (!node.contains(event.target as Node)) {
				closeDropdown();
			}
		}

		document.addEventListener('click', handleClick, true);

		return {
			destroy() {
				document.removeEventListener('click', handleClick, true);
			}
		};
	}
</script>

<!-- Link snippet for reusable navigation links -->
{#snippet link(
	href: string,
	label: string,
	target: string = '_blank',
	rel: string = 'noopener noreferrer'
)}
	<a {href} {target} {rel} class="text-14 text-semibold link-snippet">
		{label}
	</a>
{/snippet}

<header class="header">
	<GitbutlerLogoLink />

	<nav class="header-nav">
		<section class="flex gap-20">
			<!-- link to the downloads page -->
			{@render link(
				jsonLinks.resources.downloads.url,
				jsonLinks.resources.downloads.label,
				'_self',
				''
			)}
			{@render link(jsonLinks.resources.documentation.url, jsonLinks.resources.documentation.label)}
			{@render link(jsonLinks.resources.blog.url, jsonLinks.resources.blog.label)}

			<div class="social-dropdown" use:clickOutside>
				<button type="button" class="text-14 text-semibold social-button" onclick={toggleDropdown}>
					<span class="social-button-label">Community</span>
					<span class="social-button-arrow" class:rotated={isDropdownOpen}>▼</span>
				</button>

				{#if isDropdownOpen}
					<div class="dropdown-menu" in:fly={{ y: -10, duration: 150 }}>
						{#each socialLinks as socialLink}
							<a
								href={socialLink.url}
								target="_blank"
								rel="noopener noreferrer"
								class="text-14 text-semibold social-link"
							>
								{socialLink.label}

								<span class="social-link-arrow">↗</span>
							</a>
						{/each}
					</div>
				{/if}
			</div>
		</section>
		<HeaderAuthSection />
	</nav>
</header>

<style lang="scss">
	.header {
		display: flex;
		grid-column: narrow-start / -2;
		justify-content: space-between;
		padding-top: 48px;

		@media (--mobile-viewport) {
			padding-top: 38px;
		}

		@media (--tablet-viewport) {
			padding-top: 20px;
		}
	}

	.header-nav {
		display: flex;
		align-items: center;
		gap: 32px;

		@media (--mobile-viewport) {
			gap: 24px;
		}
	}

	.link-snippet {
		text-decoration: none;
		transition:
			color var(--transition-fast),
			text-decoration var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
			text-decoration: underline;
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

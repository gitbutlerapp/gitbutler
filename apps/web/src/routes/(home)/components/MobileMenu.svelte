<script lang="ts">
	import HeaderAuthSection from '$lib/components/HeaderAuthSection.svelte';
	import linkJson from '$lib/data/links.json';
	import { scale } from 'svelte/transition';

	let isMenuOpen = $state(false);

	function toggleMenu() {
		isMenuOpen = !isMenuOpen;
	}

	// Simple scroll lock using CSS class
	$effect(() => {
		if (typeof document !== 'undefined') {
			document.body.classList.toggle('menu-open', isMenuOpen);
		}

		return () => {
			if (typeof document !== 'undefined') {
				document.body.classList.remove('menu-open');
			}
		};
	});
</script>

<button
	type="button"
	class="mobile-menu-button"
	class:is-open={isMenuOpen}
	onclick={toggleMenu}
	aria-label={isMenuOpen ? 'Close menu' : 'Open menu'}
	aria-expanded={isMenuOpen}
></button>

{#if isMenuOpen}
	<div class="mobile-menu" transition:scale={{ start: 0.95, duration: 150 }}>
		<div class="mobile-menu__content">
			<HeaderAuthSection />

			<div class="stack-v gap-40">
				<nav class="mobile-nav">
					<a
						href={linkJson.resources.source.url}
						target="_blank"
						rel="noopener noreferrer"
						class="mobile-link"
					>
						{linkJson.resources.source.label}
					</a>
					<a href={linkJson.resources.downloads.url} target="_self" class="mobile-link">
						{linkJson.resources.downloads.label}
					</a>
					<a
						href={linkJson.resources.documentation.url}
						target="_blank"
						rel="noopener noreferrer"
						class="mobile-link">{linkJson.resources.documentation.label}</a
					>
					<a
						href={linkJson.resources.blog.url}
						target="_blank"
						rel="noopener noreferrer"
						class="mobile-link">Blog</a
					>
					<a
						href={linkJson.resources.jobs.url}
						target="_blank"
						rel="noopener noreferrer"
						class="mobile-link">{linkJson.resources.jobs.label}</a
					>
				</nav>

				<div class="mobile-socials">
					{#each Object.values(linkJson.social) as social}
						<a
							href={social.url}
							target="_blank"
							rel="noopener noreferrer"
							class="mobile-social-link"
						>
							{social.label}
						</a>
					{/each}
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	.mobile-menu-button {
		display: none;
		z-index: 200;
		position: relative;
		width: 40px;
		height: 40px;
		border: none;
		background: none;
		cursor: pointer;

		&::before,
		&::after {
			position: absolute;
			left: 0;
			width: 100%;
			height: 0.094rem;
			background-color: var(--clr-text-1);
			content: '';
			transition:
				transform 0.15s ease,
				top 0.15s ease,
				bottom 0.15s ease;
		}

		&::before {
			top: 14px;
		}
		&::after {
			bottom: 14px;
		}

		&:hover {
			&::before {
				top: 13px;
			}
			&::after {
				bottom: 13px;
			}
		}

		@media (--tablet-viewport) {
			display: block;
		}

		&.is-open {
			&::before {
				top: 50%;
				transform: translateY(-50%);
			}
			&::after {
				bottom: 50%;
				transform: translateY(50%);
			}
		}
	}

	.mobile-menu__content {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		width: 100%;
		height: 100%;
		padding: 44px;

		@media (--mobile-viewport) {
			padding: 24px;
		}
	}

	.mobile-menu {
		z-index: 100;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100dvh; /* Dynamic viewport height for mobile browsers */
		backdrop-filter: blur(16px);
		background: color-mix(in srgb, var(--clr-bg-2) 50%, transparent);
	}

	.mobile-nav {
		display: flex;
		flex-direction: column;
		padding: 8px 0;
	}

	.mobile-link {
		display: block;
		width: fit-content;
		font-size: 48px;
		font-family: var(--fontfamily-accent);
		text-underline-offset: 4px;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
		}
	}

	.mobile-socials {
		display: flex;
		flex-wrap: wrap;
		gap: 16px;
	}

	.mobile-social-link {
		font-size: 18px;
		text-underline-offset: 2px;

		&:hover {
			text-decoration: underline;
		}
	}
</style>

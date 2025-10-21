<script lang="ts">
	interface Props {
		items: {
			title: string;
			description: string;
			link?: string;
			icon?: string;
		}[];
	}

	const { items }: Props = $props();
</script>

{#snippet cardContent(title: string, description: string, icon?: string)}
	{#if icon}
		<div class="feature-item__icon">
			{@html icon}
		</div>
	{/if}
	<div class="m-bottom-8">
		<h3>{title}</h3>
	</div>
	<div>
		<p>{description}</p>
	</div>
{/snippet}

<section class="features">
	{#each items as item}
		{#if item.link}
			<a class="feature-item" href={item.link} target="_blank" rel="noopener noreferrer">
				<div class="features__link-indicator">
					<div class="features__link-indicator-brakets">[</div>
					<div class="features__link-indicator-text">Learn More</div>
					<div class="features__link-indicator-arrow">&#8599;</div>
					<div class="features__link-indicator-brakets">]</div>
				</div>

				{@render cardContent(item.title, item.description, item.icon)}
			</a>
		{:else}
			<div class="feature-item">
				{@render cardContent(item.title, item.description, item.icon)}
			</div>
		{/if}
	{/each}
</section>

<style>
	.features {
		display: grid;
		position: relative;
		grid-template-columns: repeat(3, 1fr);
		grid-column: full-start / full-end;
		margin-top: 40px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
	}

	.features::before {
		z-index: 0;
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		left: 0;
		background-image: radial-gradient(circle, var(--clr-text-3) 1px, transparent 1px);
		background-size: 8px 8px;
		content: '';
		opacity: 0.3;
		pointer-events: none;
	}

	.features__link-indicator-text {
		max-width: 0;
		overflow: hidden;
		white-space: nowrap;
		transition:
			max-width 0.3s ease,
			margin-right 0.3s ease;
	}

	.features__link-indicator-brakets {
		transition: opacity 0.2s ease;
	}

	.features__link-indicator-arrow {
		transition: transform 0.2s ease;
	}

	.features__link-indicator {
		display: flex;
		position: absolute;
		top: 16px;
		right: 16px;
		align-items: center;
		font-size: 12px;
		opacity: 0.5;
	}

	.feature-item {
		display: flex;
		z-index: 1;
		position: relative;
		flex-direction: column;
		padding: 24px;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		text-decoration: none;
		transition: background-color var(--transition-fast);

		p,
		h3 {
			display: inline;
			box-decoration-break: clone;
			-webkit-box-decoration-break: clone;
			width: fit-content;
			background: var(--clr-bg-2);
			transition: background-color var(--transition-fast);
		}

		h3 {
			max-width: 90%;
			margin-bottom: 8px;
			font-weight: 600;
			font-size: 18px;
			line-height: 1.2;
		}

		p {
			flex-grow: 1;
			font-size: 15px;
			line-height: 1.5;
			opacity: 0.8;
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);

			p,
			h3 {
				background-color: var(--clr-bg-1-muted);
			}

			& .features__link-indicator-text {
				max-width: 200px;
				margin-right: 4px;
			}

			& .features__link-indicator-brakets {
				opacity: 0;
			}

			& .features__link-indicator-arrow {
				transform: translate(2px, -2px) scale(1.2);
			}
		}
	}

	.feature-item__icon {
		height: 24px;
		margin-bottom: 12px;
		color: var(--clr-text-2);

		& :global(svg) {
			width: auto;
			height: 100%;
		}
	}

	/* Desktop: 3-column layout (default) */
	.feature-item:nth-child(3n) {
		border-right: none;
	}

	.feature-item:nth-last-child(-n + 3) {
		border-bottom: none;
	}

	/* Tablet: 2-column layout */
	@media (max-width: 1024px) {
		.features {
			grid-template-columns: repeat(2, 1fr);
		}

		/* Reset desktop rules first */
		.feature-item:nth-child(3n) {
			border-right: 1px solid var(--clr-border-2);
		}

		.feature-item:nth-last-child(-n + 3) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		/* Apply tablet-specific rules */
		.feature-item:nth-child(2n) {
			border-right: none;
		}

		.feature-item:nth-last-child(-n + 2) {
			border-bottom: none;
		}
	}

	/* Mobile: single-column layout */
	@media (max-width: 740px) {
		.features {
			grid-template-columns: 1fr;
			grid-column: narrow-start / off-center;
			margin-top: 24px;
		}

		/* Reset all previous rules */
		.feature-item:nth-child(3n),
		.feature-item:nth-child(2n) {
			border-right: none;
		}

		.feature-item:nth-last-child(-n + 3),
		.feature-item:nth-last-child(-n + 2) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		/* Apply mobile-specific rules */
		.feature-item {
			border-right: none;
			border-bottom: 1px solid var(--clr-border-2);
		}

		.feature-item:last-child {
			border-bottom: none;
		}
	}
</style>

<script lang="ts">
	interface Props {
		items: {
			title: string;
			description: string;
			link?: string;
		}[];
	}

	const { items }: Props = $props();
</script>

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

				<h3>{item.title}</h3>
				<p>{item.description}</p>
			</a>
		{:else}
			<div class="feature-item">
				<h3>{item.title}</h3>
				<p>{item.description}</p>
			</div>
		{/if}
	{/each}
</section>

<style>
	.features {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		grid-column: full-start / full-end;
		margin-top: 40px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-xl);
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
		position: relative;
		flex-direction: column;
		padding: 24px;
		overflow: hidden;
		border-right: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);
		text-decoration: none;

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
		}

		/* Remove right border for last column (every 3rd item) and very last item */
		&:last-child,
		&:nth-child(3n) {
			border-right: none;
		}

		/* Remove bottom border for items in the last row (last 3 items) */
		&:nth-last-child(-n + 3) {
			border-bottom: none;
		}

		&:hover {
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
</style>

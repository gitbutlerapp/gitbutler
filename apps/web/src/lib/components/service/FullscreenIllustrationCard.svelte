<script lang="ts">
	import walkininSvg from '$lib/assets/splash-illustrations/walkin.svg?raw';
	import type { Snippet } from 'svelte';

	interface Props {
		title: Snippet;
		children: Snippet;
		footer?: Snippet;
	}

	let { title, children, footer }: Props = $props();
</script>

<div class="instruction-page">
	<div class="instruction-card__container">
		<div class="instruction-content">
			<h1 class="text-serif-42 instruction-content__title">
				{@render title()}
			</h1>

			{@render children()}

			<div class="instruction-content__footer text-12 text-body">
				{@render footer?.()}
			</div>
		</div>

		<div class="instruction-content__illustration">
			{@html walkininSvg}
		</div>
	</div>
</div>

<style lang="postcss">
	.instruction-page {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.instruction-card__container {
		display: flex;
		width: 100%;
		max-width: 1048px;
		overflow: hidden;
		border-radius: var(--radius-xl);
	}

	.instruction-content {
		display: flex;
		flex: 4;
		flex-direction: column;
		justify-content: center;
		width: 100%;
		padding: 50px 80px 30px;
		background-color: var(--clr-bg-1);
	}

	.instruction-content__title {
		margin-bottom: 40px;
	}

	.instruction-content__footer {
		margin-top: 30px;
		color: var(--clr-text-2);

		:global(a) {
			text-decoration: underline;

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}

	.instruction-content__illustration {
		display: flex;
		flex: 4;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px;
		background-color: var(--clr-illustration-bg);

		:global(svg) {
			max-width: 400px;
		}
	}

	@media (max-width: 1020px) {
		.instruction-content {
			flex: 5;
			padding: 40px 40px 20px;
		}
	}

	@media (max-width: 840px) {
		.instruction-content__illustration {
			display: none;
		}

		.instruction-content__title {
			margin-bottom: 24px;
		}
	}

	@media (max-width: 480px) {
		.instruction-content {
			padding: 30px 20px 20px;
		}
	}
</style>

<script lang="ts">
	import CtaButtons from "../components/CtaButtons.svelte";
</script>

<section class="cta-section">
	<div class="cta-section__content">
		<div class="chip-free-opensource text-15 text-semibold">>_ Free and Open Source</div>
		<h2 class="cta-section__header">Ready to <i>Level Up Your</i> Workflow?</h2>
		<p class="cta-section__description text-16 text-body">
			Join thousands of developers who've already made the switch.
			<br />
			GitButler is compatible with your existing Git repositories.
		</p>

		<CtaButtons darkMode />
	</div>

	<div class="grainy-bg grainy-1"></div>
	<div class="grainy-bg grainy-2"></div>
</section>

<style>
	.cta-section {
		display: grid;
		position: relative;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		padding: 58px 32px;
		overflow: hidden;
		border-radius: var(--radius-xl);
		background-color: var(--clr-theme-pop-element);
	}

	.cta-section__content {
		z-index: 1;
		position: relative;
		grid-column: narrow-start / narrow-end;
	}

	.chip-free-opensource {
		display: inline-block;
		padding: 8px 12px;
		border-radius: 100px;
		background: var(--clr-theme-pop-on-element);
		color: var(--clr-core-pop-40);
	}

	.cta-section__header {
		max-width: 600px;
		margin-top: 16px;
		color: var(--clr-theme-pop-on-element);
		font-size: 82px;
		line-height: 1;
		font-family: var(--font-accent);
	}

	.cta-section__description {
		max-width: 600px;
		margin-top: 16px;
		color: var(--clr-theme-pop-on-element);
		opacity: 0.8;
	}

	.grainy-bg {
		background-image: url("/images/grainy-gradient-dark.png");
		opacity: 0.14;
	}

	.grainy-1 {
		top: -30%;
		left: -20%;
	}
	.grainy-2 {
		right: -30%;
		bottom: -120%;
	}

	@media (--mobile-viewport) {
		.cta-section {
			padding: 24px;
		}
		.cta-section__header {
			font-size: 48px;
			text-wrap: balance;
		}
	}
</style>

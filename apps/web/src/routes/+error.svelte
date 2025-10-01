<script lang="ts">
	import { page } from '$app/state';
	import Footer from '$lib/components/marketing/Footer.svelte';
</script>

<section class="wrapper">
	<div class="title">
		{page.status}
	</div>
	<div class="subtitle">Error</div>
</section>

<Footer />

<style lang="scss">
	.wrapper {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 50vmin;
	}

	.title {
		margin-top: 30px;
		font-size: min(10vw, 16cqw);
		font-family: var(--fontfamily-accent);

		@media (max-width: 700px) {
			margin-bottom: 30px;
			font-size: 16cqw;
			// margin-top: 20px;
		}
	}
	.subtitle {
		max-width: 400px;
		margin-bottom: 30px;
		color: var(--clr-black);
		font-weight: 400;
		font-size: 22px;
		line-height: 130%;
		text-wrap: balance;
		opacity: 0.7;

		@media (max-width: 800px) {
			font-size: 18px;
		}
	}
</style>

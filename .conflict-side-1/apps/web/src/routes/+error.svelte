<script>
	import Footer from '$home/sections/Footer.svelte';
	import { page } from '$app/stores';
</script>

<section class="wrapper">
	<div class="title">
		{$page.status}
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
		font-family: 'PP Editorial New', serif;
		font-size: min(10vw, 16cqw);
		margin-top: 30px;

		@media (max-width: 700px) {
			font-size: 16cqw;
			margin-bottom: 30px;
			// margin-top: 20px;
		}
	}
	.subtitle {
		font-size: 22px;
		color: var(--clr-black);
		font-weight: 400;
		line-height: 130%;
		opacity: 0.7;
		max-width: 400px;
		text-wrap: balance;
		margin-bottom: 30px;

		@media (max-width: 800px) {
			font-size: 18px;
		}
	}
</style>

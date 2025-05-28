<script lang="ts">
	import CtaBlock from '$home/components/CtaBlock.svelte';
	import * as jsonLinks from '$lib/data/links.json';
</script>

<section class="hero" id="hero">
	<div class="content">
		<h1 class="title">Git Branching,<br /><i>Refined</i></h1>
		<p class="caption">A Git client for simultaneous branches on top of your existing workflow.</p>

		<div class="desktop-Ctas">
			<CtaBlock
				secondButton={{
					label: 'View Source',
					url: jsonLinks.resources.github.url
				}}
			/>
		</div>
	</div>

	<div class="preview">
		<video
			class="video"
			autoplay
			loop
			muted
			playsinline
			src="https://d3brppdydubvmf.cloudfront.net/videos/web/main-preview.mp4#t=0.1"
		></video>
		<div class="video-shadow"></div>
	</div>

	<div class="mobile-Ctas">
		<CtaBlock
			secondButton={{
				label: 'View Source',
				url: jsonLinks.resources.github.url
			}}
		/>
	</div>
</section>

<style lang="scss">
	.hero {
		display: flex;
		position: relative;
		padding-top: 60px;
		padding-bottom: 90px;
		gap: 80px;

		@media (max-width: 1100px) {
			flex-direction: column;
			padding-top: 40px;
			padding-bottom: 60px;
			gap: 50px;
		}

		@media (max-width: 500px) {
			padding-top: 20px;
			padding-bottom: 40px;
			gap: 30px;
		}
	}

	.desktop-Ctas {
		display: flex;
		flex-direction: column;

		@media (max-width: 1100px) {
			display: none;
		}
	}

	.mobile-Ctas {
		display: none;

		@media (max-width: 1100px) {
			display: flex;
			flex-direction: column;
		}
	}

	.content {
		container-type: inline-size;
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.title {
		margin-top: 30px;
		margin-bottom: 40px;
		font-size: min(10vw, 16cqw);
		line-height: 92%;

		font-family:
			PP Editorial New,
			serif;

		@media (max-width: 700px) {
			margin-bottom: 30px;
			font-size: 16cqw;
		}
	}

	.caption {
		max-width: 520px;
		margin-bottom: 40px;
		color: var(--clr-dark-gray);
		font-size: 22px;
		line-height: 150%;
		text-wrap: balance;

		@media (max-width: 1100px) {
			margin-bottom: 0;
			font-size: 20px;
		}

		@media (max-width: 500px) {
			font-size: 18px;
		}
	}

	.preview {
		position: relative;
		flex: 1.2;
	}

	.video,
	.video-shadow {
		position: absolute;
		min-width: 860px;
		height: 555px;
		border-radius: 12px;
	}

	.video {
		z-index: 2;
		object-fit: cover;
		border: 1px solid var(--clr-gray);
		background-color: var(--clr-bg);

		@media (max-width: 1100px) {
			position: relative;
			width: 100%;
			height: auto;
		}
	}

	.video-shadow {
		z-index: -1;
		top: 40px;
		left: 40px;
		border-radius: 12px;
		background-image: url('/images/patterns/random-noise-1.gif');
		opacity: 0.3;

		@media (max-width: 1100px) {
			display: none;
		}
	}
</style>

<script lang="ts">
	import CtaBlock from '$home/lib/components/CtaBlock.svelte';
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
		position: relative;
		display: flex;
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
		display: flex;
		flex-direction: column;
		flex: 1;
		container-type: inline-size;
	}

	.title {
		font-size: min(10vw, 16cqw);
		margin-top: 30px;
		margin-bottom: 40px;

		@media (max-width: 700px) {
			font-size: 16cqw;
			margin-bottom: 30px;
			// margin-top: 20px;
		}
	}

	.caption {
		color: var(--clr-dark-gray);
		font-size: 22px;
		max-width: 520px;
		line-height: 150%;
		margin-bottom: 40px;
		text-wrap: balance;

		@media (max-width: 1100px) {
			font-size: 20px;
			margin-bottom: 0;
		}

		@media (max-width: 500px) {
			font-size: 18px;
			// margin-bottom: 50px;
		}
	}

	.preview {
		position: relative;
		flex: 1.2;
	}

	.video,
	.video-shadow {
		position: absolute;
		width: 860px;
		height: 555px;
		border-radius: 12px;
	}

	.video {
		z-index: 2;
		object-fit: cover;
		border: 1px solid var(--clr-gray);
		background-color: var(--clr-bg);

		@media (max-width: 1100px) {
			width: 100%;
			height: auto;
			position: relative;
		}
	}

	.video-shadow {
		top: 40px;
		left: 40px;
		background-image: url('/images/patterns/random-noise-1.gif');
		border-radius: 12px;
		z-index: -1;
		opacity: 0.3;

		@media (max-width: 1100px) {
			display: none;
		}
	}
</style>

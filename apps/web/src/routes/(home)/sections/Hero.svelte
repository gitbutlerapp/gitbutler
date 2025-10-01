<script lang="ts">
	import CtaButton from '$home/components/CtaButton.svelte';
	import contentJson from '$home/data/content.json';
	import MarketingHeader from '$lib/components/MarketingHeader.svelte';

	const heroContent = contentJson.hero;
</script>

<section class="hero">
	<MarketingHeader />
	<div class="hero-content">
		<h1 class="title">
			Git, <i>finally</i> designed for humans.
			<i class="title-caption">(And AI Agents)</i>
		</h1>
		<p class="description">
			{heroContent.description}
		</p>

		<section class="cta">
			<CtaButton />
			<a
				class="video-preview"
				href={heroContent.demo}
				target="_blank"
				rel="noopener noreferrer"
				aria-label="Watch demo video"
				on:mouseenter={(e) => e.currentTarget.querySelector('video')?.play()}
				on:mouseleave={(e) => {
					const video = e.currentTarget.querySelector('video');
					if (video) {
						video.pause();
						video.currentTime = 0;
					}
				}}
			>
				<svg
					class="play-icon"
					width="49"
					height="34"
					viewBox="0 0 49 34"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path
						d="M47.9762 5.30912C47.4126 3.21936 45.7524 1.57359 43.6438 1.01506C39.8229 0 24.5 0 24.5 0C24.5 0 9.17728 0 5.35611 1.01506C3.24758 1.57359 1.58693 3.21936 1.02378 5.30912C0 9.09688 0 17 0 17C0 17 0 24.9027 1.02378 28.6909C1.58693 30.7808 3.24758 32.4262 5.3559 32.9854C9.17708 34 24.4998 34 24.4998 34C24.4998 34 39.8227 34 43.6436 32.9854C45.7522 32.4264 47.4124 30.7808 47.976 28.6911C48.9999 24.9029 48.9999 17.0002 48.9999 17.0002C48.9999 17.0002 48.9999 9.09709 47.976 5.30933"
						fill="#FF0000"
					/>
					<path d="M19 23.5714L31.8068 16.2861L19 9V23.5714Z" fill="white" />
				</svg>

				<video src="/images/demo-preview/demo.mp4#t=0.1" loop muted playsinline></video>
			</a>
		</section>
	</div>
</section>

<style lang="scss">
	.hero {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		flex-direction: column;
		background: var(--color-hero-background);
		color: var(--color-hero-text);
	}

	.hero-content {
		display: grid;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		max-width: 700px;
		padding-top: 52px;
	}

	.title {
		margin-bottom: 32px;
		font-size: 82px;
		line-height: 1;
		font-family: var(--fontfamily-accent);
		text-wrap: balance;
	}

	.title-caption {
		display: inline-flex;
		transform: translateY(14px);
		color: var(--clr-text-2);
		font-size: 63%;
	}

	.description {
		max-width: 520px;
		color: var(--clr-text-2);
		font-size: 16px;
		line-height: 1.5;
	}

	.cta {
		display: flex;
		margin-top: 40px;
		gap: 24px;
	}

	.video-preview {
		display: inline-block;
		position: relative;
		width: 200px;
		overflow: hidden;
		border-radius: var(--radius-xl);

		video {
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 100%;
			object-fit: cover;
		}

		&:hover .play-icon {
			transform: scale(1.1);
		}
	}

	.play-icon {
		z-index: 1;
		position: absolute;
		bottom: 16px;
		left: 16px;
		transform-origin: left bottom;
		pointer-events: none;
		transition: transform 0.2s ease;
	}

	@media (--mobile-viewport) {
		.title {
			margin-bottom: 16px;
			font-size: 62px;
		}
		.cta {
			flex-direction: column;
			align-items: flex-start;
			gap: 16px;
		}
		.video-preview {
			aspect-ratio: 16 / 9;
			width: 100%;
		}
		.title-caption {
			display: block;
			width: 100%;
			margin-top: 14px;
			transform: none;
			font-size: 70%;
			text-align: right;
		}
	}
</style>

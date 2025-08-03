<script lang="ts">
	import { isMobile } from '$lib/utils/isMobile';
	import { smoothScroll } from '$lib/utils/smoothScroll';
	import { onMount } from 'svelte';

	interface Props {
		videoUrl: string;
		posterUrl: string;
		title: string;
		description: string;
		readMoreUrl: string;
	}

	let { videoUrl, posterUrl, title, description, readMoreUrl }: Props = $props();

	let videoElement = $state<HTMLVideoElement>();
	let videoCurrentTime = $state<number>();
	let progressScaleProcentage = $state(0);
	let windowWidth = $state(0);
	let isMobileBrekpoint = $derived(isMobile(windowWidth));
	let isVideoPlayingOnMobile = $state(false);

	function handleMouseEnter() {
		if (isMobileBrekpoint) return;

		videoElement?.play();
	}

	function handleMouseLeave() {
		if (isMobileBrekpoint) return;

		videoElement?.pause();
	}

	$effect(() => {
		if (videoCurrentTime) {
			const videoDuration = videoElement?.duration ?? 1;
			progressScaleProcentage = (videoCurrentTime / videoDuration) * 100;
		}
	});

	let io: IntersectionObserver;

	onMount(() => {
		const ioOptions = {
			root: null,
			rootMargin: '-30% 0% -50% 0%',
			threshold: 0
		};

		io = new IntersectionObserver((entries) => {
			entries.forEach((entry) => {
				if (entry.isIntersecting) {
					if (!isMobileBrekpoint) return;

					isVideoPlayingOnMobile = true;

					videoElement?.play();
				} else {
					if (!isMobileBrekpoint) return;

					isVideoPlayingOnMobile = false;

					videoElement?.pause();
				}
			});
		}, ioOptions);

		io.observe(videoElement as Element);

		return () => {
			io.disconnect();
		};
	});
</script>

<svelte:window bind:innerWidth={windowWidth} />

<div class="card-wrapper">
	<article class="card" onmouseenter={handleMouseEnter} onmouseleave={handleMouseLeave}>
		<div class="video-wrappper">
			<div
				class="progress-scale"
				class:showProgress={isVideoPlayingOnMobile}
				style="width: {progressScaleProcentage}%"
			></div>
			<div
				class="video-poster"
				class:hidePoster={isVideoPlayingOnMobile}
				style={`background-image: url(${posterUrl})`}
			>
				<svg
					width="33"
					height="19"
					viewBox="0 0 33 19"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<rect width="21" height="19" fill="#41B4AE" />
					<path d="M21 9.5L33 0V19L21 9.5Z" fill="#41B4AE" />
				</svg>
			</div>
			<video
				bind:this={videoElement}
				bind:currentTime={videoCurrentTime}
				class="video"
				loop
				muted
				playsinline
				preload="auto"
				src={`${videoUrl}#t=0.1`}
			></video>
		</div>
		<div class="content">
			<h3 class="title">{title}</h3>
			<p class="description">
				{description}
			</p>

			<a class="read-more-btn" href={readMoreUrl} onclick={smoothScroll}>
				<span>More</span>

				<svg
					width="10"
					height="10"
					viewBox="0 0 10 10"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
				>
					<path d="M9 1L9 9M9 9L1 9M9 9L1 1" stroke-width="1.5" />
				</svg>
			</a>
		</div>
	</article>
	<div class="shadow"></div>
</div>

<style lang="scss">
	.card-wrapper {
		position: relative;

		@media (min-width: 800px) {
			&:hover {
				.card {
					transform: translate(-8px, -8px);
					transition: transform 0.1s ease;
				}

				.read-more-btn {
					transform: translate(0, 0);
					opacity: 1;
				}

				.progress-scale {
					transform: scaleY(1) translateY(0);
					opacity: 1;
				}

				.shadow {
					transform: translate(6px, 6px);
					opacity: 0.3;
				}

				.video-poster {
					opacity: 0;
				}
			}
		}
	}

	.card {
		display: flex;
		z-index: 1;
		position: relative;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-gray);
		border-radius: 16px;
		backface-visibility: hidden;
		background-color: var(--clr-white);
		transition: transform 0.3s ease;
	}

	.video-poster {
		display: flex;
		z-index: 2;
		position: absolute;
		top: -2px;
		left: -2px;
		align-items: flex-start;
		justify-content: flex-end;
		width: calc(100% + 4px);
		height: calc(100% + 4px);
		padding: 20px;
		background-position: center;
		background-size: cover;
		background-repeat: no-repeat;
		transition: opacity 0.1s ease-in-out;
	}

	.hidePoster {
		opacity: 0;
	}

	.progress-scale {
		z-index: 2;
		position: absolute;
		bottom: 0;
		left: 0;
		width: 0;
		height: 3px;
		transform: scaleY(0.3) translateY(5px);
		background-color: var(--clr-accent);
		filter: brightness(0.8) saturate(1.6);
		opacity: 0;

		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;
	}

	.showProgress {
		transform: scaleY(1) translateY(0);
		opacity: 1;
	}

	.shadow {
		z-index: -1;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		border-radius: 16px;
		background-image: url('/images/patterns/random-noise-1.gif');
		opacity: 0;
		transition:
			transform 0.1s ease,
			opacity 0.1s ease;
	}

	.video-wrappper {
		position: relative;
		aspect-ratio: 240/180;
		width: 100%;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-gray);
	}

	.video {
		z-index: 1;
		position: absolute;
		top: -2px;
		left: -2px;
		width: calc(100% + 4px);
		height: calc(100% + 4px);
		object-fit: cover;
	}

	.content {
		position: relative;
		flex: 1;
		padding: 30px 30px 40px 30px;
		background-color: var(--clr-white);
	}

	.title {
		margin-bottom: 16px;
		font-weight: 500;
		font-size: 28px;
		text-transform: uppercase;
	}

	.description {
		color: var(--clr-dark-gray);
		font-size: 14px;
		line-height: 160%;
	}

	.read-more-btn {
		display: inline-flex;
		position: absolute;
		right: 0;
		bottom: 0;
		align-items: center;
		justify-content: center;
		padding: 20px;
		gap: 8px;
		transform: translate(0, 10px);
		border: none;
		outline: none;
		background: none;
		text-decoration: none;
		cursor: pointer;

		opacity: 0;
		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;

		span {
			color: color-mix(in srgb, var(--clr-accent), var(--clr-black) 20%);
			font-weight: 500;
			font-size: 14px;
		}

		svg {
			transition: transform 0.1s ease-in-out;
		}

		path {
			stroke: color-mix(in srgb, var(--clr-accent), var(--clr-black) 20%);
		}

		&:hover {
			svg {
				transform: translate(2px, 2px);
			}
		}
	}
</style>

<script lang="ts">
	import { isMobile } from '$home/lib/utils/isMobile';
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
					opacity: 1;
					transform: translate(0, 0);
				}

				.progress-scale {
					opacity: 1;
					transform: scaleY(1) translateY(0);
				}

				.shadow {
					opacity: 0.3;
					transform: translate(6px, 6px);
				}

				.video-poster {
					opacity: 0;
				}
			}
		}
	}

	.card {
		z-index: 1;
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 16px;
		height: 100%;
		background-color: var(--clr-white);
		border: 1px solid var(--clr-gray);
		overflow: hidden;
		backface-visibility: hidden;
		transition: transform 0.3s ease;
	}

	.video-poster {
		z-index: 2;
		position: absolute;
		top: -2px;
		left: -2px;
		width: calc(100% + 4px);
		height: calc(100% + 4px);
		display: flex;
		align-items: flex-start;
		justify-content: flex-end;
		padding: 20px;
		background-size: cover;
		background-position: center;
		background-repeat: no-repeat;
		transition: opacity 0.1s ease-in-out;
	}

	.hidePoster {
		opacity: 0;
	}

	.progress-scale {
		position: absolute;
		z-index: 2;
		bottom: 0;
		left: 0;
		width: 0;
		height: 3px;
		background-color: var(--clr-accent);
		filter: brightness(0.8) saturate(1.6);
		opacity: 0;
		transform: scaleY(0.3) translateY(5px);

		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;
	}

	.showProgress {
		opacity: 1;
		transform: scaleY(1) translateY(0);
	}

	.shadow {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background-image: url('/images/patterns/random-noise-1.gif');
		border-radius: 16px;
		z-index: -1;
		opacity: 0;
		transition:
			transform 0.1s ease,
			opacity 0.1s ease;
	}

	.video-wrappper {
		width: 100%;
		aspect-ratio: 240/180;
		position: relative;
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
		flex: 1;
		position: relative;
		background-color: var(--clr-white);
		padding: 30px 30px 40px 30px;
	}

	.title {
		font-size: 28px;
		font-weight: 500;
		text-transform: uppercase;
		margin-bottom: 16px;
	}

	.description {
		color: var(--clr-dark-gray);
		font-size: 14px;
		line-height: 160%;
	}

	.read-more-btn {
		position: absolute;
		bottom: 0;
		right: 0;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 8px;
		background: none;
		border: none;
		outline: none;
		padding: 20px;
		text-decoration: none;

		opacity: 0;
		transform: translate(0, 10px);
		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;

		span {
			font-size: 14px;
			font-weight: 500;
			color: color-mix(in srgb, var(--clr-accent), var(--clr-black) 20%);
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

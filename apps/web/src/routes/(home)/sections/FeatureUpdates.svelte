<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
	import {
		fetchPlaylistVideos,
		extractPlaylistId,
		getEmbedUrl,
		getHighQualityThumbnail,
		getFallbackThumbnail,
		type YouTubePlaylist
	} from '$lib/youtube';
	import { onMount } from 'svelte';

	// Constants
	const PLAYLIST_URL =
		'https://youtube.com/playlist?list=PLNXkW_le40U7IH8qA5VPN6f01oC25LOj4&si=IRZbd5aBoNLWDH5g';
	const SCROLL_AMOUNT = 400;
	const SCROLL_UPDATE_DELAY = 300;

	// State
	let playlist: YouTubePlaylist | null = $state(null);
	let isLoading = $state(true);
	let error = $state('');
	let carousel: HTMLElement | undefined = $state();
	let canScrollLeft = $state(false);
	let canScrollRight = $state(false);
	let playingVideo: string | null = $state(null);

	// Scroll utilities
	function updateScrollState() {
		if (!carousel) return;
		canScrollLeft = carousel.scrollLeft > 0;
		canScrollRight = carousel.scrollLeft < carousel.scrollWidth - carousel.clientWidth;
	}

	function scroll(direction: 'left' | 'right') {
		if (!carousel) return;
		const scrollAmount = direction === 'left' ? -SCROLL_AMOUNT : SCROLL_AMOUNT;
		carousel.scrollBy({ left: scrollAmount, behavior: 'smooth' });
		setTimeout(updateScrollState, SCROLL_UPDATE_DELAY);
	}

	// Video control
	function handleVideoPlay(videoId: string) {
		playingVideo = videoId;
	}

	function openPlaylist() {
		window.open(PLAYLIST_URL, '_blank');
	}

	// Image error handling
	function handleImageError(event: Event, videoId: string) {
		const img = event.currentTarget as HTMLImageElement;
		img.src = getFallbackThumbnail(videoId);
	}

	// Keyboard interaction
	function handleKeydown(event: KeyboardEvent, videoId: string) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			handleVideoPlay(videoId);
		}
	}

	// Effects
	$effect(() => {
		if (!carousel) return;

		const timeoutId = setTimeout(updateScrollState, 0);
		carousel.addEventListener('scroll', updateScrollState);

		return () => {
			clearTimeout(timeoutId);
			carousel?.removeEventListener('scroll', updateScrollState);
		};
	});

	$effect(() => {
		if (playlist && carousel) {
			setTimeout(updateScrollState, 100);
		}
	});

	// Data loading
	onMount(async () => {
		try {
			const playlistId = extractPlaylistId(PLAYLIST_URL);
			if (!playlistId) {
				throw new Error('Invalid playlist URL');
			}

			playlist = await fetchPlaylistVideos(playlistId);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load videos';
			console.error('Error loading playlist:', err);
		} finally {
			isLoading = false;
		}
	});
</script>

<section class="feature-updates">
	<SectionHeader>
		<i>Feature</i> updates

		{#snippet buttons()}
			<ArrowButton showArrow={false} label="All demos" onclick={openPlaylist} />
			<ArrowButton onclick={() => scroll('left')} reverseDirection disabled={!canScrollLeft} />
			<ArrowButton onclick={() => scroll('right')} disabled={!canScrollRight} />
		{/snippet}
	</SectionHeader>

	{#if isLoading}
		<div class="loading-state">
			<p>Loading videos...</p>
		</div>
	{:else if error}
		<div class="loading-state">
			<h3>¯\_(ツ)_/¯</h3>
			<p>Unable to load videos: {error}</p>
			<p>
				Please check our <a href={PLAYLIST_URL} target="_blank" rel="noopener">YouTube playlist</a>
				directly.
			</p>
		</div>
	{:else if playlist && playlist.videos.length > 0}
		<div class="video-content">
			<div class="video-carousel__container">
				<div class="video-carousel__scroll" bind:this={carousel}>
					{#each playlist?.videos ?? [] as video (video.id)}
						<div class="video-embed">
							{#if playingVideo === video.videoId}
								<iframe
									src={`${getEmbedUrl(video.videoId)}?autoplay=1`}
									title={video.title}
									frameborder="0"
									allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
									referrerpolicy="strict-origin-when-cross-origin"
									allowfullscreen
								></iframe>
							{:else}
								<div
									class="video-preview"
									role="button"
									tabindex="0"
									onclick={() => handleVideoPlay(video.videoId)}
									onkeydown={(e) => handleKeydown(e, video.videoId)}
								>
									<img
										src={getHighQualityThumbnail(video.videoId)}
										alt={video.title}
										loading="lazy"
										onerror={(e) => handleImageError(e, video.videoId)}
									/>
									<div class="play-button">
										<svg
											width="70"
											height="50"
											viewBox="0 0 70 50"
											fill="none"
											xmlns="http://www.w3.org/2000/svg"
										>
											<path
												d="M68.5375 8.50243C67.7324 5.51706 65.3606 3.16596 62.3484 2.36806C56.8899 0.917969 35 0.917969 35 0.917969C35 0.917969 13.1104 0.917969 7.6516 2.36806C4.6394 3.16596 2.26705 5.51706 1.46255 8.50243C0 13.9135 0 25.2037 0 25.2037C0 25.2037 0 36.4933 1.46255 41.905C2.26705 44.8906 4.6394 47.2412 7.6513 48.0399C13.1101 49.4894 34.9997 49.4895 34.9997 49.4895C34.9997 49.4895 56.8896 49.4894 62.3481 48.0399C65.3603 47.2415 67.7321 44.8907 68.5372 41.9053C70 36.4936 70 25.204 70 25.204C70 25.204 70.0003 13.9135 68.5375 8.50243Z"
												fill="#FF0000"
											/>
											<path
												d="M27.1426 34.5937L45.438 24.1861L27.1426 13.7773V34.5937Z"
												fill="white"
											/>
										</svg>
									</div>
								</div>
							{/if}
						</div>
					{/each}
				</div>
			</div>
		</div>
	{/if}
</section>

<style>
	.feature-updates {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.video-content {
		grid-column: full-start / full-end;
		width: calc(100% + 80px);
		margin-left: -40px;
	}

	.video-carousel__container {
		position: relative;
		width: 100%;
	}

	.video-carousel__container::before,
	.video-carousel__container::after {
		z-index: 1;
		position: absolute;
		top: 0;
		width: 40px;
		height: 100%;
		content: '';
		pointer-events: none;
	}

	.video-carousel__container::before {
		left: 0;
		background: linear-gradient(to right, var(--clr-bg-2), transparent);
	}

	.video-carousel__container::after {
		right: 0;
		background: linear-gradient(to left, var(--clr-bg-2), transparent);
	}

	.video-carousel__scroll {
		display: flex;
		overflow-x: auto;
		gap: 1rem;
		scroll-behavior: smooth;
		scroll-padding: 0 40px;
		scroll-snap-type: x mandatory;
		scrollbar-width: none;
		-ms-overflow-style: none;
	}

	.video-carousel__scroll::-webkit-scrollbar {
		display: none;
	}

	.video-embed {
		flex: 0 0 auto;
		aspect-ratio: 16 / 9;
		width: 560px;
		overflow: hidden;
		border-radius: 16px;

		scroll-snap-align: start;

		&:first-child {
			margin-left: 40px;
		}

		&:last-child {
			margin-right: 40px;
		}
	}

	.video-embed iframe {
		width: 100%;
		height: 100%;
		border: none;
	}

	.video-preview {
		position: relative;
		width: 100%;
		height: 100%;
		cursor: pointer;
		transition: transform 0.2s ease;
	}

	.video-preview:hover {
		transform: scale(1.02);

		.play-button {
			transform: translate(-50%, -50%) scale(0.9);
		}
	}

	.video-preview img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		border-radius: 8px;
	}

	.play-button {
		display: flex;
		position: absolute;
		top: 50%;
		left: 50%;
		align-items: center;
		justify-content: center;
		transform: translate(-50%, -50%);
		transition: transform 0.2s ease;
	}

	.loading-state {
		display: flex;
		grid-column: narrow-start / narrow-end;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 40px 0;
		border-radius: 16px;
		background-color: var(--clr-bg-3);
		color: var(--clr-text-2);
		font-size: 16px;
		text-align: center;

		h3 {
			margin-bottom: 16px;
			font-size: 32px;
		}

		p {
			font-size: 14px;
			line-height: 1.5;
		}

		a {
			color: var(--clr-link);
			text-decoration: underline;
		}
	}
</style>

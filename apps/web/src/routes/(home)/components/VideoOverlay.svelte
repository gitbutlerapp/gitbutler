<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		videoUrl: string;
		onClose: () => void;
	}

	let { videoUrl, onClose }: Props = $props();

	// Convert YouTube URL to embed URL with autoplay
	function getYouTubeEmbedUrl(url: string): string {
		// If it's already an embed URL, just add/update autoplay parameter
		if (url.includes('/embed/')) {
			const urlObj = new URL(url);
			urlObj.searchParams.set('autoplay', '1');
			return urlObj.toString();
		}

		// Otherwise, convert watch/short URL to embed with autoplay
		const youtubeRegex = /(?:youtube\.com\/watch\?v=|youtu\.be\/)([^?&]+)/;
		const match = url.match(youtubeRegex);
		if (match) {
			return `https://www.youtube.com/embed/${match[1]}?autoplay=1`;
		}

		return url;
	}

	const embedUrl = $derived(getYouTubeEmbedUrl(videoUrl));

	onMount(() => {
		// Handle escape key
		function handleKeydown(e: KeyboardEvent) {
			if (e.key === 'Escape') {
				onClose();
			}
		}

		window.addEventListener('keydown', handleKeydown);

		return () => {
			window.removeEventListener('keydown', handleKeydown);
		};
	});

	function handleBackdropClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onClose();
		}
	}

	function handleBackdropKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onClose();
		}
	}
</script>

<div
	class="overlay"
	onclick={handleBackdropClick}
	onkeydown={handleBackdropKeydown}
	role="dialog"
	aria-modal="true"
	tabindex="-1"
>
	<div class="video-container">
		<iframe
			src={embedUrl}
			title="Demo video"
			frameborder="0"
			allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
			allowfullscreen
		></iframe>
	</div>
</div>

<style lang="scss">
	.overlay {
		display: flex;
		z-index: 9999;
		position: fixed;
		top: 0;
		left: 0;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		backdrop-filter: blur(8px);
		background-color: rgba(0, 0, 0, 0.7);
		animation: fadeIn 0.2s ease;
	}

	@keyframes fadeIn {
		from {
			opacity: 0;
		}
		to {
			opacity: 1;
		}
	}

	.video-container {
		position: relative;
		aspect-ratio: 16 / 9;
		width: 90%;
		max-width: 1200px;
		animation: scaleIn 0.3s ease;
		animation-delay: 0.1s;
	}

	@keyframes scaleIn {
		from {
			transform: scale(0.9);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
	}

	iframe {
		width: 100%;
		height: 100%;
		border: none;
		border-radius: var(--radius-xl);
	}

	@media (--mobile-viewport) {
		.video-container {
			width: 95%;
		}
	}
</style>

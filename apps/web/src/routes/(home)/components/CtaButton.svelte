<script lang="ts">
	import ImagineCanvas from '$home/sections/ImagineCanvas.svelte';
	import jsonLinks from '$lib/data/links.json';
	import osIcons from '$lib/data/os-icons.json';
	import { latestClientVersion } from '$lib/store';
	import { onMount } from 'svelte';

	let detectedOS = $state('');
	let selectedDownload = $state(jsonLinks.downloads.appleSilicon);
	let tiltX = $state(0);
	let tiltY = $state(0);
	let isHovering = $state(false);

	// OS detection mapping for cleaner logic
	const osDetectionMap = [
		{
			check: (ua: string) => ua.includes('mac'),
			os: 'macOS',
			// There is no good way of determining the arminess of a mac, so we
			// should just assume appleSilicon.
			getDownload: () => jsonLinks.downloads.appleSilicon
		},
		{
			check: (ua: string) => ua.includes('windows'),
			os: 'Windows',
			getDownload: () => jsonLinks.downloads.windowsMsi
		},
		{
			check: (ua: string) => ua.includes('linux'),
			os: 'Linux',
			getDownload: () => jsonLinks.downloads.linuxDeb
		}
	];

	// Function to detect OS and set appropriate download
	function detectOS() {
		const userAgent = navigator.userAgent.toLowerCase();

		const detected = osDetectionMap.find((config) => config.check(userAgent));

		if (detected) {
			detectedOS = detected.os;
			selectedDownload = detected.getDownload();
		} else {
			// Default to macOS Apple Silicon if OS can't be detected
			detectedOS = 'macOS';
			selectedDownload = jsonLinks.downloads.appleSilicon;
		}
	}

	// Function to get OS-specific SVG icon
	function getOSIcon(os: string): string {
		const osKey = os.toLowerCase() as keyof typeof osIcons;
		return osIcons[osKey] || osIcons.macos;
	}

	// Tilt effect handlers (only for non-touch devices)
	function handleMouseMove(e: MouseEvent) {
		// Skip tilt effect on touch devices
		if (window.matchMedia('(pointer: coarse)').matches) return;

		const target = e.currentTarget as HTMLElement;
		const rect = target.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const y = e.clientY - rect.top;

		// Calculate tilt based on mouse position (max 6 degrees)
		const centerX = rect.width / 2;
		const centerY = rect.height / 2;
		const maxTilt = 6;

		tiltY = ((x - centerX) / centerX) * maxTilt;
		tiltX = ((centerY - y) / centerY) * maxTilt;
	}

	function handleMouseEnter() {
		// Skip hover effect on touch devices
		if (window.matchMedia('(pointer: coarse)').matches) return;
		isHovering = true;
	}

	function handleMouseLeave() {
		isHovering = false;
		tiltX = 0;
		tiltY = 0;
	}

	onMount(() => {
		detectOS();
	});
</script>

<!-- DESKTOP -->
<a
	class="download-btn desktop"
	href={selectedDownload.url}
	onmouseenter={handleMouseEnter}
	onmouseleave={handleMouseLeave}
	onmousemove={handleMouseMove}
	style:transform="perspective(1000px) rotateX({tiltX}deg) rotateY({tiltY}deg) translateY({isHovering
		? '-2px'
		: '0'}) scale({isHovering ? 1.02 : 1})"
>
	<div class="download-btn__title">
		<span class="download-btn__title">DOWNLOAD for {detectedOS}</span>

		<svg
			class="download-btn-icon"
			viewBox="0 0 22 22"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path d={getOSIcon(detectedOS)} fill="currentColor" />
		</svg>
	</div>

	<span class="download-btn__version">Open Beta {$latestClientVersion}</span>

	<div class="download-btn__canvas-cover"></div>
	<ImagineCanvas />
</a>

<!-- MOBILE -->
<a class="download-btn mobile" href={jsonLinks.resources.downloads.url}>
	<div class="download-btn__title">
		<span class="download-btn__title">DOWNLOAD <i>the</i> app</span>
	</div>
	<span class="download-btn__version">Open Beta {$latestClientVersion}</span>

	<div class="download-btn__canvas-cover"></div>
	<ImagineCanvas />
</a>

<style lang="postcss">
	.download-btn {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 20px 28px 24px;
		overflow: hidden;
		gap: 4px;
		transform-style: preserve-3d;
		border: 1px solid color-mix(in srgb, var(--clr-theme-pop-element) 50%, transparent);
		border-radius: var(--radius-xl);
		background-color: var(--clr-theme-pop-soft);
		color: var(--clr-theme-pop-text);
		transition:
			transform 0.15s ease-out,
			color 0.15s ease,
			background-color 0.15s ease,
			border-color 0.15s ease,
			box-shadow 0.2s ease;
		will-change: transform;

		&:hover {
			background-color: hsl(
				from var(--clr-theme-pop-soft) h s calc(l - (var(--opacity-btn-solid-hover) * 50))
			);
			box-shadow: 0 12px 26px color-mix(in srgb, var(--clr-theme-pop-element) 30%, transparent);
		}
	}

	.download-btn.mobile {
		display: none;
		align-items: center;
		width: 100%;
	}

	.download-btn__title {
		display: flex;
		z-index: 1;
		align-items: center;
		gap: 8px;
		pointer-events: none;
	}

	.download-btn__title {
		z-index: 2;
		font-size: 42px;
		line-height: 1;
		font-family: var(--font-accent);
	}

	.download-btn-icon {
		width: 28px;
		height: 28px;
	}

	.download-btn__version {
		z-index: 2;
		margin-top: 2px;
		font-size: 14px;
		font-family: var(--font-mono);
		text-transform: uppercase;
		opacity: 0.6;
		pointer-events: none;
	}

	.download-btn__canvas-cover {
		z-index: 1;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: radial-gradient(
			ellipse at center,
			var(--clr-theme-pop-bg-muted) 40%,
			rgba(255, 255, 255, 0) 100%
		);
		pointer-events: none;
	}

	@media (--mobile-viewport) {
		.download-btn.desktop {
			display: none;
		}
		.download-btn.mobile {
			display: flex;
		}
		.download-btn {
			padding: 20px;
		}
		.download-btn__title {
			font-size: 38px;
			text-align: center;
		}
	}
</style>

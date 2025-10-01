<script lang="ts">
	import jsonLinks from '$lib/data/links.json';
	import osIcons from '$lib/data/os-icons.json';
	import { latestClientVersion } from '$lib/store';
	import { onMount } from 'svelte';

	let detectedOS = $state('');
	let selectedDownload = $state(jsonLinks.downloads.appleSilicon);

	// OS detection mapping for cleaner logic
	const osDetectionMap = [
		{
			check: (ua: string) => ua.includes('mac'),
			os: 'macOS',
			getDownload: (ua: string) =>
				ua.includes('arm64') || ua.includes('aarch64')
					? jsonLinks.downloads.appleSilicon
					: jsonLinks.downloads.intelMac
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
			selectedDownload = detected.getDownload(userAgent);
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

	onMount(() => {
		detectOS();
	});
</script>

<!-- DESKTOP -->
<a class="download-btn desktop" href={selectedDownload.url}>
	<div class="download-btn-title">
		<span class="download-btn-title">DOWNLOAD for {detectedOS}</span>

		<svg
			class="download-btn-icon"
			viewBox="0 0 22 22"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path d={getOSIcon(detectedOS)} fill="currentColor" />
		</svg>
	</div>

	<span class="download-btn-version">Open Beta {$latestClientVersion}</span>
</a>

<!-- MOBILE -->
<a class="download-btn mobile" href={jsonLinks.resources.downloads.url}>
	<div class="download-btn-title">
		<span class="download-btn-title">DOWNLOAD <i>the</i> app</span>
	</div>
	<span class="download-btn-version">Open Beta {$latestClientVersion}</span>
</a>

<style lang="scss">
	.download-btn {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 20px 28px 24px;
		overflow: hidden;
		gap: 4px;
		border: 1px solid var(--clr-scale-pop-60);
		border-radius: var(--radius-xl);
		background-color: var(--clr-theme-pop-soft);
		color: var(--clr-theme-pop-on-soft);
		transition:
			transform 0.15s ease,
			color 0.15s ease,
			background-color 0.15s ease,
			border-color 0.15s ease,
			drop-shadow 0.2s ease;

		&:hover {
			transform: translateY(-2px) scale(1.02);
			background-color: var(--clr-theme-pop-soft-hover);
			box-shadow: 0 16px 16px color-mix(in srgb, var(--clr-theme-pop-element) 10%, transparent);
		}
	}

	.download-btn.mobile {
		display: none;
		align-items: center;
		width: 100%;
	}

	.download-btn-title {
		display: flex;
		z-index: 1;
		align-items: center;
		gap: 8px;
		pointer-events: none;
	}

	.download-btn-title {
		font-size: 42px;
		line-height: 1;
		font-family: var(--fontfamily-accent);
	}

	.download-btn-icon {
		width: 28px;
		height: 28px;
	}

	.download-btn-version {
		z-index: 1;
		margin-top: 2px;
		font-size: 14px;
		font-family: var(--fontfamily-mono);
		text-transform: uppercase;
		opacity: 0.6;
		pointer-events: none;
	}

	.canvas-container {
		z-index: 0;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
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
		.download-btn-title {
			font-size: 38px;
			text-align: center;
		}
	}
</style>

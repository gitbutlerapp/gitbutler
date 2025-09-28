<script lang="ts">
	import CanvasDitheringEffect from '$home/components/CanvasDitheringEffect.svelte';
	import * as osIcons from '$home/data/os-icons.json';
	import * as jsonLinks from '$lib/data/links.json';
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

<a class="download-btn" href={selectedDownload.url}>
	<div class="download-btn-title-wrapper">
		<span class="download-btn-title">Download for {detectedOS}</span>

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

	<CanvasDitheringEffect />
</a>

<style lang="scss">
	.download-btn {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 16px 28px 24px;
		overflow: hidden;
		border: 1px solid var(--clr-scale-pop-60);
		border-radius: var(--radius-xl);
		background-color: var(--clr-theme-pop-soft);
		color: var(--clr-theme-pop-on-soft);
	}

	.download-btn-title-wrapper {
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
</style>

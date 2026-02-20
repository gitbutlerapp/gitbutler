<script lang="ts">
	import osIcons from '$lib/data/os-icons.json';
	import { RELEASE_OS_ORDER } from '$lib/utils/releaseUtils';
	import type { Build } from '$lib/types/releases';

	interface Props {
		builds: Build[];
	}

	const { builds }: Props = $props();

	function getBuildDisplayName(build: Build): string {
		if (build.os === 'darwin') {
			const platform = build.platform.toLowerCase();
			if (
				build.arch === 'aarch64' ||
				platform.includes('silicon') ||
				platform.includes('aarch64')
			) {
				return 'Apple Silicon';
			}
			if (build.arch === 'x86_64' || platform.includes('intel') || platform.includes('x86_64')) {
				return 'Intel Mac';
			}
			return platform.startsWith('macos') ? build.platform : `macOS ${build.platform}`;
		}

		if (build.os === 'windows') {
			const file = build.file.toLowerCase();
			if (file.includes('msi')) return 'Windows (MSI)';
			if (file.includes('exe')) return 'Windows (EXE)';
			return 'Windows';
		}

		if (build.os === 'linux') {
			const arch = build.arch === 'aarch64' ? 'ARM64' : 'Intel';
			const file = build.file.toLowerCase();
			if (file.includes('appimage')) return `Linux ${arch} (AppImage)`;
			if (file.includes('deb')) return `Linux ${arch} (Deb)`;
			if (file.includes('rpm')) return `Linux ${arch} (RPM)`;
			return `Linux ${arch} (CLI)`;
		}

		return build.platform;
	}

	function getIconKey(os: string): keyof typeof osIcons {
		if (os === 'darwin') return 'macos';
		if (os === 'windows' || os === 'linux') return os;
		return 'linux';
	}

	function getBuildSortRank(build: Build): number {
		if (build.os === 'darwin') {
			if (build.arch === 'aarch64') return 0;
			if (build.arch === 'x86_64') return 1;
			return 2;
		}

		if (build.os === 'windows') {
			const file = build.file.toLowerCase();
			if (file.includes('msi')) return 0;
			if (file.includes('exe')) return 1;
			return 2;
		}

		if (build.os === 'linux') {
			const archRank = build.arch === 'aarch64' ? 1 : 0;
			const file = build.file.toLowerCase();
			let packageRank = 3;
			if (file.includes('appimage')) packageRank = 0;
			else if (file.includes('deb')) packageRank = 1;
			else if (file.includes('rpm')) packageRank = 2;

			return archRank * 10 + packageRank;
		}

		return 99;
	}

	function compareBuildsWithinOs(a: Build, b: Build): number {
		const rankDiff = getBuildSortRank(a) - getBuildSortRank(b);
		if (rankDiff !== 0) return rankDiff;

		const labelDiff = getBuildDisplayName(a).localeCompare(getBuildDisplayName(b));
		if (labelDiff !== 0) return labelDiff;

		return a.url.localeCompare(b.url);
	}

	const groupedBuilds = $derived.by(() => {
		const seenBuilds = new Set<string>();
		const uniqueBuilds = builds.filter((build) => {
			if (seenBuilds.has(build.url)) return false;
			seenBuilds.add(build.url);
			return true;
		});

		const grouped = uniqueBuilds.reduce(
			(acc, build) => {
				if (!acc[build.os]) acc[build.os] = [];
				acc[build.os].push(build);
				return acc;
			},
			{} as Record<string, Build[]>
		);

		for (const osBuilds of Object.values(grouped)) {
			osBuilds.sort(compareBuildsWithinOs);
		}

		const osOrder: readonly string[] = RELEASE_OS_ORDER;
		return Object.entries(grouped).sort(([a], [b]) => {
			const aIndex = osOrder.indexOf(a);
			const bIndex = osOrder.indexOf(b);
			const aOrder = aIndex === -1 ? osOrder.length : aIndex;
			const bOrder = bIndex === -1 ? osOrder.length : bIndex;
			return aOrder - bOrder;
		});
	});
</script>

<div class="download-links">
	{#each groupedBuilds as [os, osBuilds]}
		<div class="download-category">
			<svg class="download-icon" viewBox="0 0 22 22" fill="none" xmlns="http://www.w3.org/2000/svg">
				<path d={osIcons[getIconKey(os)]} fill="currentColor" />
			</svg>
			<div class="download-options">
				{#each osBuilds as build}
					<a href={build.url} class="download-link">
						{getBuildDisplayName(build)}
					</a>
				{/each}
			</div>
		</div>
	{/each}
</div>

<style>
	.download-links {
		display: flex;
		flex-wrap: wrap;
		padding: 32px;
		gap: 24px;
		background-image: radial-gradient(var(--clr-border-2) 1px, transparent 1px);
		background-size: 6px 6px;
	}

	.download-category {
		display: flex;
		flex-direction: column;
		min-width: 150px;
		gap: 12px;
	}

	.download-icon {
		width: 22px;
		height: 22px;
	}

	.download-options {
		display: flex;
		flex-wrap: wrap;
		gap: 14px;
	}

	.download-link {
		width: fit-content;
		background-color: var(--clr-bg-2);
		font-size: 14px;
		font-family: var(--font-mono);
		text-decoration: underline;
		text-underline-offset: 2px;
		transition: all 0.1s ease;

		&:hover {
			text-decoration: underline wavy;
			text-decoration-color: var(--clr-theme-pop-element);
		}
	}

	@media (--mobile-viewport) {
		.download-links {
			padding: 16px;
		}
	}
</style>

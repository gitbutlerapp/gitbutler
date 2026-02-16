<script lang="ts">
	// Re-export the downloads page component
	// This allows /releases to show the same content as /downloads
	//
	// NOTE: This URL (https://gitbutler.com/releases) is referenced by the CLI
	// after performing updates, so maintaining this route is important for
	// providing users with release notes and download information.
	import DownloadsPage from "../downloads/+page.svelte";
	import type { Release } from "$lib/types/releases";
	import type { LatestReleaseBuilds } from "$lib/utils/releaseUtils";

	interface Props {
		data: {
			releases: Release[];
			nightlies: Release[];
			latestRelease: Release;
			latestReleaseBuilds: LatestReleaseBuilds;
		};
	}

	const { data }: Props = $props();
</script>

<DownloadsPage {data} />

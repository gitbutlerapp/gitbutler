import { parse } from "yaml";
import type { PageServerLoad } from "./$types";

async function fetchNextBuild(fetch: typeof globalThis.fetch, path: string, manifest: string) {
	try {
		const base = `https://releases.gitbutler.com/lite/${path}/`;
		const response = await fetch(`${base}${manifest}`, { signal: AbortSignal.timeout(5000) });
		if (!response.ok) throw new Error(`Manifest returned ${response.status}`);
		const data = parse(await response.text());
		if (typeof data?.version !== "string" || !Array.isArray(data.files)) {
			throw new Error("Invalid nightly manifest");
		}
		const downloads: { url: string; label: string }[] = [];
		for (const file of data.files) {
			if (typeof file?.url !== "string") continue;
			const label = file.url.endsWith(".dmg")
				? "DMG"
				: file.url.endsWith(".AppImage")
					? "AppImage"
					: file.url.endsWith(".deb")
						? "DEB"
						: null;
			const url = new URL(file.url, base);
			if (label && url.href.startsWith(base)) downloads.push({ url: url.href, label });
		}
		const releasedAt = typeof data.releaseDate === "string" ? data.releaseDate : null;
		return downloads.length ? { version: data.version as string, releasedAt, downloads } : null;
	} catch (error) {
		console.error(`Failed to fetch Next nightly for ${path}:`, error);
		return null;
	}
}

// eslint-disable-next-line func-style
export const load: PageServerLoad = async ({ fetch }) => {
	const [mac, linux, linuxArm64] = await Promise.all([
		fetchNextBuild(fetch, "mac/arm64", "nightly-mac.yml"),
		fetchNextBuild(fetch, "linux/x64", "nightly-linux.yml"),
		fetchNextBuild(fetch, "linux/arm64", "nightly-linux-arm64.yml"),
	]);
	return { nextNightly: { mac, linux, linuxArm64 } };
};

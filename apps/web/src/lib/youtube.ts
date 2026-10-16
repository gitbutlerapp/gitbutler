import { env } from "$env/dynamic/public";

export interface YouTubeVideo {
	id: string;
	title: string;
	description: string;
	thumbnail: string;
	publishedAt: string;
	channelTitle: string;
	videoId: string;
	url: string;
}

export interface YouTubePlaylist {
	id: string;
	title: string;
	description: string;
	videos: YouTubeVideo[];
}

/**
 * Extract playlist ID from YouTube playlist URL
 */
export function extractPlaylistId(url: string): string | null {
	try {
		const urlObj = new URL(url);
		return urlObj.searchParams.get("list");
	} catch {
		return null;
	}
}

/**
 * Get YouTube video URL from video ID
 */
export function getVideoUrl(videoId: string): string {
	return `https://www.youtube.com/watch?v=${videoId}`;
}

/**
 * Get YouTube video embed URL from video ID
 */
export function getEmbedUrl(videoId: string): string {
	return `https://www.youtube.com/embed/${videoId}`;
}

/**
 * Get high-quality YouTube thumbnail URL
 * maxresdefault.jpg (1280x720) - highest quality, may not exist for all videos
 * hqdefault.jpg (480x360) - high quality, more reliable
 * mqdefault.jpg (320x180) - medium quality (default)
 */
export function getHighQualityThumbnail(videoId: string): string {
	// Try maxres first for highest quality
	return `https://img.youtube.com/vi/${videoId}/maxresdefault.jpg`;
}

/**
 * Get fallback thumbnail URL if high quality fails
 */
export function getFallbackThumbnail(videoId: string): string {
	return `https://img.youtube.com/vi/${videoId}/hqdefault.jpg`;
}

type APIYouTubeVideo = {
	description: string;
	published_at: string;
	thumbnail_url: string;
	title: string;
	video_id: string;
};

function mapAPIToYouTubeVideo(apiVideo: APIYouTubeVideo): YouTubeVideo {
	return {
		id: apiVideo.video_id,
		title: apiVideo.title,
		description: apiVideo.description,
		thumbnail: getHighQualityThumbnail(apiVideo.video_id),
		publishedAt: apiVideo.published_at,
		channelTitle: "GitButler", // Static since we know the channel
		videoId: apiVideo.video_id,
		url: getVideoUrl(apiVideo.video_id),
	};
}

/**
 * Fetches videos from a YouTube playlist without requiring an API key
 * Uses a combination of RSS feed and fallback data
 */
export async function fetchPlaylistVideos(playlistId: string): Promise<YouTubePlaylist> {
	try {
		const response = await fetch(`${env.PUBLIC_APP_HOST}api/youtube/playlist`, {
			// Add timeout to prevent hanging
			signal: AbortSignal.timeout(10000),
		});

		if (response.ok) {
			const data = (await response.json()) as { videos: APIYouTubeVideo[] };
			const videos = data.videos.map(mapAPIToYouTubeVideo);
			return {
				id: playlistId,
				title: "GitButler Feature Updates",
				description: "Latest GitButler tutorials, feature demonstrations, and updates",
				videos,
			};
		}
	} catch (error) {
		console.warn("Failed to fetch from RSS feed:", error);
	}

	// Fallback to hardcoded playlist data for the specific GitButler playlist
	return getGitButlerPlaylistFallback(playlistId);
}

/**
 * Fallback data with actual GitButler video information
 * This can be updated manually when new videos are added to the playlist
 */
function getGitButlerPlaylistFallback(playlistId: string): YouTubePlaylist {
	const videos: YouTubeVideo[] = [
		{
			id: "DsejRSPitEs",
			title: "How to run Local LLMs",
			description:
				"Scott Chacon demonstrates how to run Meta's new Muse Glimmer open weight model via Ollama on a Mac M4 Studio with 36GB of RAM, using it to build a Twitter clone entirely on a free, open, local LLM.\n\nIf you would like to see how to run a good, free model on relatively normal commodity Apple hardware to do real work, check it out.\n\nM5 Max article: https://blog.gitbutler.com/local-llm-gauntlet\nOllama: https://ollama.com/\nMuse Glimmer model: https://ollama.com/library/muse-glimmer:30b-mlx\nGlimmer on Huggingface: https://huggingface.co/meta-models/Muse-Glimmer-30B",
			thumbnail: "https://img.youtube.com/vi/DsejRSPitEs/maxresdefault.jpg",
			publishedAt: "2026-09-10T13:12:11+00:00",
			channelTitle: "GitButler",
			videoId: "DsejRSPitEs",
			url: getVideoUrl("DsejRSPitEs"),
		},
		{
			id: "yho-9hpvETM",
			title: "GitHub Stacked PRs",
			description:
				"Scott explains the new GitHub Stacked Pull Requests feature and demonstrates how to use GitButler to easily create and manage stacked PRs on GitHub with no extra tooling.",
			thumbnail: "https://img.youtube.com/vi/yho-9hpvETM/maxresdefault.jpg",
			publishedAt: "2026-08-11T15:42:27+00:00",
			channelTitle: "GitButler",
			videoId: "yho-9hpvETM",
			url: getVideoUrl("yho-9hpvETM"),
		},
		{
			id: "zFO210G5m7s",
			title: "The GitButler TUI",
			description:
				"Join Scott while he covers the GitButler TUI - a magical terminal user interface that makes it lightning fast to stack, restack, split, absorb and generally do anything you want to your git history instantly.\n\nDocs: https://docs.gitbutler.com/gitbutler-tui",
			thumbnail: "https://img.youtube.com/vi/zFO210G5m7s/maxresdefault.jpg",
			publishedAt: "2026-06-19T15:51:24+00:00",
			channelTitle: "GitButler",
			videoId: "zFO210G5m7s",
			url: getVideoUrl("zFO210G5m7s"),
		},
		{
			id: "p-t44CsEDw0",
			title: "GitButler CLI - Stacking Branches",
			description:
				"Join Scott while he walks through stacking branches using GitButler tooling (CLI, GUI)",
			thumbnail: "https://img.youtube.com/vi/p-t44CsEDw0/maxresdefault.jpg",
			publishedAt: "2026-03-11T14:26:25+00:00",
			channelTitle: "GitButler",
			videoId: "p-t44CsEDw0",
			url: getVideoUrl("p-t44CsEDw0"),
		},
		{
			id: "Jg8L3SbgZ3o",
			title: "Intro to the GitButler CLI",
			description:
				"Scott walks through the GitButler CLI, a smart, drop-in replacement to the Git command line tool.\n\nWebsite: 👉 https://gitbutler.com/cli\nInstall: 👉 curl -fsSL https://gitbutler.com/install.sh | sh\nDocs 👉 https://docs.gitbutler.com/cli-overview\nGitHub repo 👉 https://github.com/gitbutlerapp/gitbutler/",
			thumbnail: "https://img.youtube.com/vi/Jg8L3SbgZ3o/maxresdefault.jpg",
			publishedAt: "2026-02-05T19:25:41+00:00",
			channelTitle: "GitButler",
			videoId: "Jg8L3SbgZ3o",
			url: getVideoUrl("Jg8L3SbgZ3o"),
		},
		{
			id: "eSNCg6_AAok",
			title: "Gerrit Mode",
			description:
				"GitButler CEO Scott Chacon demonstrates GitButler's new Gerrit Mode - automatic Gerrit handling to make using the Gerrit code review system super simple. Scott shows how to enable Gerrit mode in GitButler and how to create Gerrit changes, update existing changes in both the desktop client and the CLI",
			thumbnail: "https://img.youtube.com/vi/eSNCg6_AAok/maxresdefault.jpg",
			publishedAt: "2025-11-14T18:59:49+00:00",
			channelTitle: "GitButler",
			videoId: "eSNCg6_AAok",
			url: getVideoUrl("eSNCg6_AAok"),
		},
		{
			id: "iHDF89sGI9M",
			title: "Stacking Branches with Git and GitButler",
			description:
				"GitButler CEO Scott Chacon explains how to stack branches with GitButler and vanilla Git and how to set it up properly in the GitHub Pull Requests as dependencies. We will start a new feature, do a few commits, start a new branch based off of that, do some more commits and open PRs that are dependent on each other. We'll try this out both with Git and then see a much simpler way to do the same thing with GitButler.",
			thumbnail: "https://img.youtube.com/vi/iHDF89sGI9M/maxresdefault.jpg",
			publishedAt: "2025-10-28T14:24:45+00:00",
			channelTitle: "GitButler",
			videoId: "iHDF89sGI9M",
			url: getVideoUrl("iHDF89sGI9M"),
		},
		{
			id: "NOYK7LTFvZM",
			title: "Using Cursor Hooks for automatic version control",
			description:
				"Here we demonstrate how to use GitButler with the new Cursor Hooks functionality to automate creating branches for chat sessions and committing work as you go with smart commit messages. Never lose a step again.",
			thumbnail: "https://img.youtube.com/vi/NOYK7LTFvZM/maxresdefault.jpg",
			publishedAt: "2025-09-29T20:50:53+00:00",
			channelTitle: "GitButler",
			videoId: "NOYK7LTFvZM",
			url: getVideoUrl("NOYK7LTFvZM"),
		},
		{
			id: "JzxXNS0SfUE",
			title: "Squashing Git Commits together",
			description:
				"In this episode we'll be showing you how to take multiple Git commits and magically turn them into one. We will do this with reset, rebase and GitButler (the easy way).",
			thumbnail: "https://img.youtube.com/vi/JzxXNS0SfUE/maxresdefault.jpg",
			publishedAt: "2025-09-29T15:01:27+00:00",
			channelTitle: "GitButler",
			videoId: "JzxXNS0SfUE",
			url: getVideoUrl("JzxXNS0SfUE"),
		},
		{
			id: "ttZ3GX0sYTE",
			title: "Splitting Git Commits (the easy way)",
			description:
				"In this episode, we'll be showing you how to split a commit in Git in a few different ways. With reset, with rebase and with GitButler (the easy way).",
			thumbnail: "https://img.youtube.com/vi/ttZ3GX0sYTE/maxresdefault.jpg",
			publishedAt: "2025-09-26T16:01:22+00:00",
			channelTitle: "GitButler",
			videoId: "ttZ3GX0sYTE",
			url: getVideoUrl("ttZ3GX0sYTE"),
		},
		{
			id: "r8bmF5UpZbY",
			title: "Editing Commits - No longer a Pain in the Git",
			description:
				'Here we go over how to "fix up" a commit in both Git and GitButler. We\'ll look at how to drag and drop a modified file on a commit to amend it in GitButler and then do the same functional thing using vanilla Git via fixup commits and autosquashing rebase commands.',
			thumbnail: "https://img.youtube.com/vi/r8bmF5UpZbY/maxresdefault.jpg",
			publishedAt: "2025-09-18T11:49:34+00:00",
			channelTitle: "GitButler",
			videoId: "r8bmF5UpZbY",
			url: getVideoUrl("r8bmF5UpZbY"),
		},
		{
			id: "iJ9qJ-xcQ-U",
			title: "Uncommitting in Git and GitButler",
			description:
				"Scott talks about how to uncommit a commit, something that is not always as simple as you may think, especially uncommitting something in the middle of a series. \n\nHe demonstrates how to do it with two clicks in GitButler and then how to accomplish the same thing in vanilla Git - once just dropping the commit and then how to do it and still keep the changes in that commit in your working directory.\n\n0:22 - Uncommitting in GitButler\n1:16 - Uncommitting with the Git CLI\n2:12 - Dropping a commit in Git\n2:58 - Again, but keep the commit’s changes\n4:27 - Wrap up",
			thumbnail: "https://img.youtube.com/vi/iJ9qJ-xcQ-U/maxresdefault.jpg",
			publishedAt: "2025-09-02T13:55:23+00:00",
			channelTitle: "GitButler",
			videoId: "iJ9qJ-xcQ-U",
			url: getVideoUrl("iJ9qJ-xcQ-U"),
		},
	];

	return {
		id: playlistId,
		title: "GitButler Feature Updates",
		description: "Latest GitButler tutorials, feature demonstrations, and updates",
		videos,
	};
}

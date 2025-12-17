import { env } from '$env/dynamic/public';

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
		return urlObj.searchParams.get('list');
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
		channelTitle: 'GitButler', // Static since we know the channel
		videoId: apiVideo.video_id,
		url: getVideoUrl(apiVideo.video_id)
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
			signal: AbortSignal.timeout(10000)
		});

		if (response.ok) {
			const data = (await response.json()) as { videos: APIYouTubeVideo[] };
			const videos = data.videos.map(mapAPIToYouTubeVideo);
			return {
				id: playlistId,
				title: 'GitButler Feature Updates',
				description: 'Latest GitButler tutorials, feature demonstrations, and updates',
				videos
			};
		}
	} catch (error) {
		console.warn('Failed to fetch from RSS feed:', error);
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
			id: 'NOYK7LTFvZM',
			title: 'Using Cursor Hooks for automatic version control',
			description:
				'Here we demonstrate how to use GitButler with the new Cursor Hooks functionality to automate creating branches for chat sessions and committing work as you go with smart commit messages. Never lose a step again.',
			thumbnail: 'https://img.youtube.com/vi/NOYK7LTFvZM/maxresdefault.jpg',
			publishedAt: '2025-09-29T20:50:53+00:00',
			channelTitle: 'GitButler',
			videoId: 'NOYK7LTFvZM',
			url: getVideoUrl('NOYK7LTFvZM')
		},
		{
			id: 'JzxXNS0SfUE',
			title: 'Squashing Git Commits together',
			description:
				"In this episode we'll be showing you how to take multiple Git commits and magically turn them into one. We will do this with reset, rebase and GitButler (the easy way).",
			thumbnail: 'https://img.youtube.com/vi/JzxXNS0SfUE/maxresdefault.jpg',
			publishedAt: '2025-09-29T15:01:27+00:00',
			channelTitle: 'GitButler',
			videoId: 'JzxXNS0SfUE',
			url: getVideoUrl('JzxXNS0SfUE')
		},
		{
			id: 'ttZ3GX0sYTE',
			title: 'Splitting Git Commits (the easy way)',
			description:
				"In this episode, we'll be showing you how to split a commit in Git in a few different ways. With reset, with rebase and with GitButler (the easy way).",
			thumbnail: 'https://img.youtube.com/vi/ttZ3GX0sYTE/maxresdefault.jpg',
			publishedAt: '2025-09-26T16:01:22+00:00',
			channelTitle: 'GitButler',
			videoId: 'ttZ3GX0sYTE',
			url: getVideoUrl('ttZ3GX0sYTE')
		},
		{
			id: 'r8bmF5UpZbY',
			title: 'Editing Commits - No longer a Pain in the Git',
			description:
				'Here we go over how to "fix up" a commit in both Git and GitButler. We\'ll look at how to drag and drop a modified file on a commit to amend it in GitButler and then do the same functional thing using vanilla Git via fixup commits and autosquashing rebase commands.',
			thumbnail: 'https://img.youtube.com/vi/r8bmF5UpZbY/maxresdefault.jpg',
			publishedAt: '2025-09-18T11:49:34+00:00',
			channelTitle: 'GitButler',
			videoId: 'r8bmF5UpZbY',
			url: getVideoUrl('r8bmF5UpZbY')
		},
		{
			id: 'iJ9qJ-xcQ-U',
			title: 'Uncommitting in Git and GitButler',
			description:
				'Scott talks about how to uncommit a commit, something that is not always as simple as you may think, especially uncommitting something in the middle of a series. \n\nHe demonstrates how to do it with two clicks in GitButler and then how to accomplish the same thing in vanilla Git - once just dropping the commit and then how to do it and still keep the changes in that commit in your working directory.\n\n0:22 - Uncommitting in GitButler\n1:16 - Uncommitting with the Git CLI\n2:12 - Dropping a commit in Git\n2:58 - Again, but keep the commit’s changes\n4:27 - Wrap up',
			thumbnail: 'https://img.youtube.com/vi/iJ9qJ-xcQ-U/maxresdefault.jpg',
			publishedAt: '2025-09-02T13:55:23+00:00',
			channelTitle: 'GitButler',
			videoId: 'iJ9qJ-xcQ-U',
			url: getVideoUrl('iJ9qJ-xcQ-U')
		}
	];

	return {
		id: playlistId,
		title: 'GitButler Feature Updates',
		description: 'Latest GitButler tutorials, feature demonstrations, and updates',
		videos
	};
}

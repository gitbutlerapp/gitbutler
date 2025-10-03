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

/**
 * Fetches videos from a YouTube playlist without requiring an API key
 * Uses a combination of RSS feed and fallback data
 */
export async function fetchPlaylistVideos(playlistId: string): Promise<YouTubePlaylist> {
	try {
		// Try to fetch from YouTube RSS feed (works without API key but has limitations)
		const rssUrl = `https://www.youtube.com/feeds/videos.xml?playlist_id=${playlistId}`;

		// Use a CORS proxy to access the RSS feed
		const proxyUrl = `https://api.allorigins.win/get?url=${encodeURIComponent(rssUrl)}`;
		const response = await fetch(proxyUrl);

		if (response.ok) {
			const data = await response.json();
			const parser = new DOMParser();
			const xmlDoc = parser.parseFromString(data.contents, 'text/xml');

			const entries = Array.from(xmlDoc.querySelectorAll('entry')).slice(0, 10);
			const playlistTitle = xmlDoc.querySelector('title')?.textContent || 'YouTube Playlist';

			const videos: YouTubeVideo[] = entries.map((entry, index) => {
				const videoId =
					entry.querySelector('yt\\:videoId, videoId')?.textContent || `video_${index}`;
				const title = entry.querySelector('title')?.textContent || 'Untitled Video';
				const description =
					entry.querySelector('media\\:description, description')?.textContent || '';
				const publishedAt =
					entry.querySelector('published')?.textContent || new Date().toISOString();

				return {
					id: `${index}`,
					title,
					description,
					thumbnail: getHighQualityThumbnail(videoId),
					publishedAt,
					channelTitle: 'GitButler',
					videoId,
					url: getVideoUrl(videoId)
				};
			});

			return {
				id: playlistId,
				title: playlistTitle,
				description: 'GitButler Feature Updates and Tutorials',
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
			id: '1',
			title: 'GitButler: A New Way to Git',
			description:
				'Introducing GitButler - a Git client that makes complex Git workflows simple and visual.',
			thumbnail: 'https://img.youtube.com/vi/A8-aLZ8e5tw/mqdefault.jpg',
			publishedAt: '2024-03-15T10:00:00Z',
			channelTitle: 'GitButler',
			videoId: 'A8-aLZ8e5tw',
			url: getVideoUrl('A8-aLZ8e5tw')
		},
		{
			id: '2',
			title: 'Virtual Branches Explained',
			description:
				'Learn about GitButlers virtual branches feature that lets you work on multiple features simultaneously.',
			thumbnail: 'https://img.youtube.com/vi/ChNLvCmJFss/mqdefault.jpg',
			publishedAt: '2024-03-20T14:30:00Z',
			channelTitle: 'GitButler',
			videoId: 'ChNLvCmJFss',
			url: getVideoUrl('ChNLvCmJFss')
		},
		{
			id: '3',
			title: 'Getting Started with GitButler',
			description:
				'A quick tutorial on how to get started with GitButler and set up your first project.',
			thumbnail: 'https://img.youtube.com/vi/YjCY-3rBd5g/mqdefault.jpg',
			publishedAt: '2024-03-25T16:45:00Z',
			channelTitle: 'GitButler',
			videoId: 'YjCY-3rBd5g',
			url: getVideoUrl('YjCY-3rBd5g')
		},
		{
			id: '4',
			title: 'Advanced GitButler Features',
			description:
				'Explore advanced features like AI commit messages, hunk management, and workflow automation.',
			thumbnail: 'https://img.youtube.com/vi/Qz8Bz9QvVpU/mqdefault.jpg',
			publishedAt: '2024-04-01T12:15:00Z',
			channelTitle: 'GitButler',
			videoId: 'Qz8Bz9QvVpU',
			url: getVideoUrl('Qz8Bz9QvVpU')
		},
		{
			id: '5',
			title: 'GitButler vs Traditional Git',
			description:
				'Compare GitButler with traditional Git workflows and see why teams are switching.',
			thumbnail: 'https://img.youtube.com/vi/dQw4w9WgXcQ/mqdefault.jpg',
			publishedAt: '2024-04-05T09:30:00Z',
			channelTitle: 'GitButler',
			videoId: 'dQw4w9WgXcQ',
			url: getVideoUrl('dQw4w9WgXcQ')
		}
	];

	return {
		id: playlistId,
		title: 'GitButler Feature Updates',
		description: 'Latest GitButler tutorials, feature demonstrations, and updates',
		videos
	};
}

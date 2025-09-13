<script lang="ts">
	import { formatDate } from '$lib/utils/formatDate';

	interface BlogPost {
		title: string;
		url: string;
		feature_image: string;
		published_at: string;
		custom_excerpt: string;
		primary_author: {
			name: string;
		};
	}

	let posts = $state<BlogPost[]>([]);

	async function fetchRSSFeed() {
		try {
			const response = await fetch('https://blog.gitbutler.com/rss/3');
			const text = await response.text();
			const parser = new DOMParser();
			const xml = parser.parseFromString(text, 'text/xml');
			const items = xml.querySelectorAll('item');

			const parsedPosts: BlogPost[] = [];
			items.forEach((item) => {
				const title = item.querySelector('title')?.textContent || '';
				const url = item.querySelector('link')?.textContent || '';
				const pubDate = item.querySelector('pubDate')?.textContent || '';
				const description = item.querySelector('description')?.textContent || '';

				// Extract author from the author tag with format "email (Name)"
				const authorText = item.querySelector('author')?.textContent || '';
				const authorMatch = authorText.match(/\(([^)]+)\)/);
				const creator = authorMatch ? authorMatch[1] : 'GitButler Team';

				// Extract image from enclosure tag
				const enclosure = item.querySelector('enclosure');
				const feature_image =
					enclosure?.getAttribute('url') ||
					'https://blog.gitbutler.com/content/images/2023/10/gitbutler-og.png';

				// Extract excerpt from description, removing HTML tags
				const custom_excerpt = description.replace(/<[^>]*>/g, '').substring(0, 200) + '...';

				parsedPosts.push({
					title,
					url,
					feature_image,
					published_at: pubDate,
					custom_excerpt,
					primary_author: { name: creator }
				});
			});

			posts = parsedPosts;
		} catch (error) {
			console.error('Failed to fetch RSS feed:', error);
		}
	}

	$effect(() => {
		fetchRSSFeed();
	});
</script>

<section class="posts-preview" id="blog">
	<div class="posts-left">
		<h2 class="title">From the blog</h2>
		<p class="caption">Recent news & whatnot from the GitButler team.</p>

		<a class="main-post" href={posts?.[0]?.url}>
			<img src={posts?.[0]?.feature_image} alt="" class="main-post__image" />
			<div class="main-post__content">
				<div class="main-post__content__title-wrap">
					<h3 class="post-title">
						{posts?.[0]?.title}
					</h3>
					<span class="post-title-caption">
						{formatDate(posts?.[0]?.published_at ?? '')} by {posts?.[0]?.primary_author?.name}
					</span>
				</div>
				<div class="main-post__content__caption-wrap">
					<p class="post-caption">{posts?.[0]?.custom_excerpt}</p>
				</div>
			</div>
		</a>
	</div>
	<div class="posts-right">
		<a class="secondary-post" href={posts?.[1]?.url}>
			<img src={posts?.[1]?.feature_image} alt="" class="secondary-post__image" />
			<div class="secondary-post__content">
				<h3 class="post-title">{posts?.[1]?.title}</h3>
				<span class="post-title-caption">
					{formatDate(posts?.[1]?.published_at ?? '')} by {posts?.[1]?.primary_author?.name}
				</span>
			</div>
		</a>
		<a class="secondary-post" href={posts?.[2]?.url}>
			<img src={posts?.[2]?.feature_image} alt="" class="secondary-post__image" />
			<div class="secondary-post__content">
				<h3 class="post-title">{posts?.[2]?.title}</h3>
				<span class="post-title-caption">
					{formatDate(posts?.[2]?.published_at ?? '')} by {posts?.[2]?.primary_author?.name}
				</span>
			</div>
		</a>
	</div>
</section>

<style lang="scss">
	.posts-preview {
		display: flex;
		margin-bottom: 80px;
		gap: 35px;

		@media (max-width: 800px) {
			flex-direction: column;
			gap: 20px;
		}
	}

	.title {
		margin-bottom: 16px;
		color: var(--clr-black);
		font-weight: 400;
		font-size: 64px;
		line-height: 90%;
		font-family: var(--fontfamily-accent);

		@media (max-width: 800px) {
			font-size: 52px;
		}
	}

	.caption {
		max-width: 400px;
		margin-bottom: 30px;
		color: var(--clr-black);
		font-weight: 400;
		font-size: 22px;
		line-height: 130%;
		text-wrap: balance;
		opacity: 0.7;

		@media (max-width: 800px) {
			font-size: 18px;
		}
	}

	.posts-left {
		display: flex;
		flex: 4.5;
		flex-direction: column;
	}

	.posts-right {
		display: flex;
		flex: 3.5;
		flex-direction: column;
		justify-content: flex-end;
		gap: 40px;

		@media (max-width: 800px) {
			gap: 30px;
		}
	}

	.post-title {
		margin-bottom: 8px;
		color: var(--clr-black);
		font-weight: 500;
		font-size: 24px;
		line-height: 110%;
		text-decoration: underline;
		text-transform: uppercase;
		text-wrap: balance;
		transition:
			color 0.1s ease-in-out,
			filter 0.1s ease-in-out;
	}

	.post-title-caption {
		color: var(--clr-black);
		font-size: 16px;
		font-family: var(--fontfamily-accent);
		opacity: 0.4;
	}

	.post-caption {
		color: var(--clr-black);
		font-size: 15px;
		line-height: 140%;
		opacity: 0.8;
	}

	// Main post

	.main-post {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-gray);
		border-radius: 16px;
		text-decoration: none;
		cursor: pointer;
		transition:
			transform 0.1s ease-in-out,
			background-color 0.1s ease-in-out;

		&:hover {
			// transform: translateY(-2px);
			background-color: color-mix(in srgb, var(--clr-gray), var(--clr-white) 70%);

			.main-post__image {
				filter: brightness(1.05);
			}

			.post-title {
				color: var(--clr-accent);
				filter: brightness(0.6);
			}
		}
	}

	.main-post__content {
		display: flex;
		padding: 30px;
		gap: 20px;

		@media (max-width: 1300px) {
			flex-direction: column;
		}

		@media (max-width: 500px) {
			padding: 20px;
		}
	}

	.main-post__content__title-wrap {
		display: flex;
		flex: 2;
		flex-direction: column;
	}

	.main-post__content__caption-wrap {
		flex: 3;
	}

	.main-post__image {
		width: 100%;
		height: 400px;
		object-fit: cover;
		object-position: center;
		// filter: saturate(0.8);
		transition: filter 0.1s ease-in-out;
	}

	// Secondary post
	.secondary-post {
		display: flex;
		flex-direction: column;
		gap: 20px;
		text-decoration: none;
		cursor: pointer;

		&:hover {
			.secondary-post__image {
				filter: brightness(1.05);
			}

			.post-title {
				color: var(--clr-accent);
				filter: brightness(0.6);
			}
		}
	}

	.secondary-post__content {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.secondary-post__image {
		width: 100%;
		height: 240px;
		object-fit: cover;
		border-radius: 16px;
		// filter: saturate(0.8);
		transition: filter 0.1s ease-in-out;
	}
</style>

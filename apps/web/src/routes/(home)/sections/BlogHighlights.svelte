<script lang="ts">
	import ArrowButton from '$home/components/ArrowButton.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
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
			const response = await fetch('https://blog.gitbutler.com/rss/3/featured');
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

<section class="blog-highlights">
	<SectionHeader
		>From <i>the</i> Blog

		{#snippet buttons()}
			<ArrowButton
				label="Read more"
				onclick={() => window.open('https://blog.gitbutler.com', '_blank')}
			/>
		{/snippet}
	</SectionHeader>

	<div class="blog-highlights__grid">
		<a href={posts?.[0]?.url} class="blog-post blog-post--featured">
			<div class="blog-post__image-container">
				<img src={posts?.[0]?.feature_image} alt="" class="blog-post__image" />
			</div>
			<div class="blog-post__content">
				<div class="blog-post__header">
					<h3 class="blog-post__title">
						{posts?.[0]?.title}
					</h3>
					<span class="blog-post__meta">
						{formatDate(posts?.[0]?.published_at ?? '')} by {posts?.[0]?.primary_author?.name}
					</span>
				</div>
				<div class="blog-post__body">
					<p class="blog-post__excerpt">{posts?.[0]?.custom_excerpt}</p>
				</div>
			</div>
		</a>

		<div class="blog-highlights__sideposts">
			<a href={posts?.[1]?.url} class="blog-post blog-post--secondary">
				<div class="blog-post__image-container">
					<img src={posts?.[1]?.feature_image} alt="" class="blog-post__image" />
				</div>
				<div class="blog-post__content">
					<div class="blog-post__header">
						<h3 class="blog-post__title">
							{posts?.[1]?.title}
						</h3>
						<span class="blog-post__meta">
							{formatDate(posts?.[1]?.published_at ?? '')} by {posts?.[1]?.primary_author?.name}
						</span>
					</div>
					<div class="blog-post__body">
						<p class="blog-post__excerpt">{posts?.[1]?.custom_excerpt}</p>
					</div>
				</div>
			</a>
			<a href={posts?.[2]?.url} class="blog-post blog-post--secondary">
				<div class="blog-post__image-container">
					<img src={posts?.[2]?.feature_image} alt="" class="blog-post__image" />
				</div>
				<div class="blog-post__content">
					<div class="blog-post__header">
						<h3 class="blog-post__title">
							{posts?.[2]?.title}
						</h3>
						<span class="blog-post__meta">
							{formatDate(posts?.[2]?.published_at ?? '')} by {posts?.[2]?.primary_author?.name}
						</span>
					</div>
					<div class="blog-post__body">
						<p class="blog-post__excerpt">{posts?.[2]?.custom_excerpt}</p>
					</div>
				</div>
			</a>
		</div>
	</div>
</section>

<style lang="postcss">
	.blog-highlights {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.blog-highlights__grid {
		display: flex;
		grid-column: full-start / full-end;
		gap: 40px;
	}

	.blog-highlights__sideposts {
		display: flex;
		flex: 3.5;
		flex-direction: column;
		gap: 40px;
	}

	.blog-post__image-container {
		width: 100%;
		overflow: hidden;
		border-radius: 16px;
	}

	.blog-post__image {
		width: 100%;
		height: 100%;
		object-fit: cover;
		object-position: center;
		transition: transform 0.15s ease;
	}

	.blog-post {
		text-decoration: none;
		transition:
			transform 0.1s ease-in-out,
			background-color 0.1s ease-in-out;
	}

	.blog-post:hover .blog-post__image {
		transform: scale(1.05);
	}

	.blog-post:hover .blog-post__title {
		text-decoration: underline wavy;
		text-decoration-color: var(--clr-theme-pop-element);
		text-decoration-thickness: 2px;
		text-underline-offset: 4px;
	}

	.blog-post--featured {
		display: flex;
		flex: 3;
		flex-direction: column;
		gap: 24px;
		border: 1px solid var(--clr-gray);
	}

	.blog-post--featured:hover {
		background-color: color-mix(in srgb, var(--clr-gray), var(--clr-white) 70%);
	}

	.blog-post--featured .blog-post__image-container {
		height: 300px;
	}

	.blog-post--secondary {
		display: flex;
		overflow: hidden;
		gap: 32px;
	}

	.blog-post--secondary .blog-post__image-container {
		align-self: flex-start;
		aspect-ratio: 4 / 3;
		max-width: 280px;
	}

	.blog-post__content {
		display: flex;
		flex: 1;
		flex-direction: column;
	}

	.blog-post__title {
		margin-bottom: 8px;
		font-size: 40px;
		line-height: 1.1;
		font-family: var(--font-accent);
		text-wrap: balance;
	}

	.blog-post__meta {
		display: block;
		margin-bottom: 12px;
		font-size: 12px;
		opacity: 0.6;
	}

	.blog-post__excerpt {
		font-size: 15px;
		line-height: 1.6;
	}

	@media (--desktop-small-viewport) {
		.blog-post--secondary {
			flex-direction: column;
			gap: 16px;

			& .blog-post__image-container {
				aspect-ratio: auto;
				width: 100%;
				max-width: none;
				height: 200px;
			}
		}
	}

	@media (--tablet-viewport) {
		.blog-highlights__grid {
			flex-direction: column;
			gap: 32px;
		}

		.blog-highlights__sideposts {
			flex: none;
			flex-direction: row;
			gap: 24px;
		}

		.blog-post--featured {
			& .blog-post__image-container {
				height: 250px;
			}
		}

		.blog-post--secondary {
			flex: 1;
			gap: 16px;

			& .blog-post__image-container {
				height: 150px;
			}
		}

		.blog-post__title {
			font-size: 38px;
		}
	}

	@media (--mobile-viewport) {
		.blog-post--featured {
			& .blog-post__image-container {
				height: 200px;
			}
		}

		.blog-highlights__sideposts {
			flex-direction: column;
			gap: 32px;
		}

		.blog-post--secondary {
			& .blog-post__image-container {
				height: 200px;
			}
		}

		.blog-post__title {
			font-size: 36px;
		}
	}
</style>

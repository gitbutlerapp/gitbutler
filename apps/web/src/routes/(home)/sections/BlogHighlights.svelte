<script lang="ts">
	import { formatDate } from '$home/utils/formatDate';
	import GhostContentAPI, { type PostsOrPages } from '@tryghost/content-api';

	const GHOST_URL = 'https://gitbutler.ghost.io';
	const GHOST_KEY = '80bbdca8b933f3d98780c7cc1b';
	const GHOST_VERSION = 'v5.0';
	let posts = $state<PostsOrPages>();

	$effect(() => {
		const api = GhostContentAPI({
			url: GHOST_URL,
			key: GHOST_KEY,
			version: GHOST_VERSION
		});
		api.posts.browse({ limit: 3, include: 'authors' }).then((data) => {
			posts = data;
		});
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
		font-family: 'PP Editorial New', serif;
		font-weight: 400;
		font-size: 64px;
		color: var(--clr-black);
		line-height: 90%;
		margin-bottom: 16px;

		@media (max-width: 800px) {
			font-size: 52px;
		}
	}

	.caption {
		font-size: 22px;
		color: var(--clr-black);
		font-weight: 400;
		line-height: 130%;
		opacity: 0.7;
		max-width: 400px;
		text-wrap: balance;
		margin-bottom: 30px;

		@media (max-width: 800px) {
			font-size: 18px;
		}
	}

	.posts-left {
		flex: 4.5;
		display: flex;
		flex-direction: column;
	}

	.posts-right {
		flex: 3.5;
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
		gap: 40px;

		@media (max-width: 800px) {
			gap: 30px;
		}
	}

	.post-title {
		font-weight: 500;
		font-size: 24px;
		color: var(--clr-black);
		line-height: 110%;
		margin-bottom: 8px;
		text-wrap: balance;
		text-transform: uppercase;
		text-decoration: underline;
		transition:
			color 0.1s ease-in-out,
			filter 0.1s ease-in-out;
	}

	.post-title-caption {
		font-family: 'PP Editorial New', serif;
		font-size: 16px;
		color: var(--clr-black);
		opacity: 0.4;
	}

	.post-caption {
		font-size: 15px;
		color: var(--clr-black);
		line-height: 140%;
		opacity: 0.8;
	}

	// Main post

	.main-post {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		border-radius: 16px;
		overflow: hidden;
		border: 1px solid var(--clr-gray);
		text-decoration: none;
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
		gap: 20px;
		padding: 30px;

		@media (max-width: 1300px) {
			flex-direction: column;
		}

		@media (max-width: 500px) {
			padding: 20px;
		}
	}

	.main-post__content__title-wrap {
		flex: 2;
		display: flex;
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
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: 20px;
		text-decoration: none;

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
		border-radius: 16px;
		object-fit: cover;
		// filter: saturate(0.8);
		transition: filter 0.1s ease-in-out;
	}
</style>

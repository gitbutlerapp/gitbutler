<script>
	import CtaBlock from '$home/components/CtaBlock.svelte';
	import * as jsonLinks from '$lib/data/links.json';
</script>

<footer class="footer" id="footer">
	<div class="banner">
		<div class="banner-content">
			<h2 class="banner-title">Start smart<br />with GitButler</h2>
			<CtaBlock
				secondButton={{
					label: 'Join Discord',
					url: jsonLinks.social.discord.url
				}}
				lightTheme
				showVideoDemo={false}
			/>
		</div>
		<div class="banner-image-wrapper">
			<img class="banner-image" src="/images/footer/app-butler.webp" alt="App Butler" />
		</div>
	</div>

	<div class="links-wrapper">
		<div class="links">
			<ul class="links-list">
				{#each Object.values(jsonLinks.resources) as resource}
					<li class="link">
						<a href={resource.url}>
							<span>{resource.label}</span>
							<svg
								class="link-arrow"
								width="14"
								height="14"
								viewBox="0 0 14 14"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M1 1L13 1M13 1L13 13M13 1L1 13"
									stroke="var(--clr-black)"
									stroke-width="1.5"
								/>
							</svg>
						</a>
					</li>
				{/each}
			</ul>
			<ul class="links-list">
				{#each Object.values(jsonLinks.social) as social}
					<li class="link">
						<a href={social.url}>
							<span>{social.label}</span>
							<svg
								class="link-arrow"
								width="14"
								height="14"
								viewBox="0 0 14 14"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M1 1L13 1M13 1L13 13M13 1L1 13"
									stroke="var(--clr-black)"
									stroke-width="1.5"
								/>
							</svg>
						</a>
					</li>
				{/each}
			</ul>
		</div>

		<div class="meta-links">
			<span class="meta-links__copyright"
				>©{new Date().getFullYear()} GitButler. All rights reserved.</span
			>
			<span class="meta-links__legal">
				<a href="/privacy">
					{jsonLinks.legal.privacyPolicy.label}
				</a>
				<span> | </span>
				<a href={jsonLinks.legal.termsOfService.url}>
					{jsonLinks.legal.termsOfService.label}
				</a>
			</span>
		</div>
	</div>
</footer>

<style lang="scss">
	.footer {
		display: flex;
		gap: 20px;
		margin-bottom: 60px;

		@media (max-width: 1300px) {
			flex-direction: column;
			gap: 20px;
			margin-bottom: 0;
		}

		@media (max-width: 800px) {
			gap: 0;
		}
	}

	.meta-links {
		display: flex;
		flex-direction: column;
		gap: 10px;
		font-size: 14px;
		color: var(--clr-black);
	}

	.meta-links__copyright {
		opacity: 0.4;
	}

	.banner {
		display: flex;
		flex: 5;
		border-radius: 16px;
		background-color: var(--clr-accent);

		@media (max-width: 900px) {
			flex-direction: column-reverse;
		}

		@media (max-width: 800px) {
			margin: 0 -20px;
			border-radius: 0;
		}

		@media (max-width: 600px) {
			padding: 20px;
		}
	}

	.banner-content {
		flex: 2.3;
		display: flex;
		padding: 30px;
		flex-direction: column;
		justify-content: space-between;
		gap: 40px;

		@media (max-width: 900px) {
			gap: 28px;
		}
	}

	.banner-title {
		font-family: 'PP Editorial New', serif;
		font-weight: 400;
		font-size: 68px;
		color: var(--clr-black);
		line-height: 100%;
		text-wrap: balance;

		@media (max-width: 600px) {
			font-size: 14cqw;
		}
	}

	.banner-image-wrapper {
		position: relative;
		overflow: hidden;
		flex: 2;

		@media (max-width: 900px) {
			min-height: 410px;
			margin-bottom: 20px;
		}
	}

	.banner-image {
		position: absolute;
		right: 56%;
		bottom: 5%;
		min-width: 390px;
		max-width: 400px;
		transform: translateX(50%);

		@media (max-width: 900px) {
			right: 49%;
		}
	}

	.links-wrapper {
		flex: 2;
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		gap: 40px;
		padding: 30px;
		background-color: var(--clr-white);
		border-radius: 16px;

		@media (max-width: 800px) {
			margin: 0 -20px;
			border-radius: 0;
		}
	}

	.links {
		display: flex;
		gap: 60px;
		justify-content: space-between;

		@media (max-width: 900px) {
			flex-direction: column;

			gap: 26px;
		}
	}

	.link-arrow {
		position: absolute;
		right: -20px;
		opacity: 0;
		transform: translate(-3px, 3px) scale(0.8);
		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;
	}

	.link {
		font-size: 20px;
		line-height: 120%;
		color: var(--clr-black);

		a {
			position: relative;

			display: inline-block;
			cursor: pointer;

			text-decoration: none;

			//span
			span {
				position: relative;
				z-index: 1;
				color: var(--clr-black);
				text-wrap: nowrap;
				text-shadow:
					-1px -1px 0 var(--clr-white),
					1px -1px 0 var(--clr-white),
					-1px 1px 0 var(--clr-white),
					1px 1px 0 var(--clr-white);
			}

			//underline
			&:after {
				content: '';
				display: block;
				position: absolute;
				z-index: 0;
				width: 100%;
				height: 1px;
				bottom: 0;
				right: 0;
				transform: scaleY(0.5);
				background-color: var(--clr-black);
				transform-origin: right;
				transition: transform 0.2s ease-in-out;
			}
		}

		&:hover {
			a {
				&:after {
					transform: scaleX(0);
				}
			}

			.link-arrow {
				opacity: 1;
				transform: translate(0, 0);
			}
		}
	}

	.links-list {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 26px;

		@media (max-width: 900px) {
			&:first-child::after {
				content: '';
				display: block;
				width: 100%;
				height: 1px;
				background-color: var(--clr-gray);
			}
		}
	}

	.meta-links__legal {
		a {
			color: var(--clr-black);
			text-decoration: none;

			&:hover {
				text-decoration: underline;
			}
		}
	}
</style>

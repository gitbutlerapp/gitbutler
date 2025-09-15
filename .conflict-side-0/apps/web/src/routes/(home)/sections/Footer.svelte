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
		margin-bottom: 60px;
		gap: 20px;

		@media (max-width: 1300px) {
			flex-direction: column;
			margin-bottom: 0;
			gap: 20px;
		}

		@media (max-width: 800px) {
			gap: 0;
		}
	}

	.meta-links {
		display: flex;
		flex-direction: column;
		gap: 10px;
		color: var(--clr-black);
		font-size: 14px;
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
		display: flex;
		flex: 2.3;
		flex-direction: column;
		justify-content: space-between;
		padding: 30px;
		gap: 40px;

		@media (max-width: 900px) {
			gap: 28px;
		}
	}

	.banner-title {
		color: var(--clr-black);
		font-weight: 400;
		font-size: 68px;
		line-height: 100%;
		font-family: var(--fontfamily-accent);
		text-wrap: balance;

		@media (max-width: 600px) {
			font-size: 14cqw;
		}
	}

	.banner-image-wrapper {
		position: relative;
		flex: 2;
		overflow: hidden;

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
		display: flex;
		flex: 2;
		flex-direction: column;
		justify-content: space-between;
		padding: 30px;
		gap: 40px;
		border-radius: 16px;
		background-color: var(--clr-white);

		@media (max-width: 800px) {
			margin: 0 -20px;
			border-radius: 0;
		}
	}

	.links {
		display: flex;
		justify-content: space-between;
		gap: 60px;

		@media (max-width: 900px) {
			flex-direction: column;

			gap: 26px;
		}
	}

	.link-arrow {
		position: absolute;
		right: -20px;
		transform: translate(-3px, 3px) scale(0.8);
		opacity: 0;
		transition:
			opacity 0.1s ease-in-out,
			transform 0.1s ease-in-out;
	}

	.link {
		color: var(--clr-black);
		font-size: 20px;
		line-height: 120%;

		a {
			display: inline-block;
			position: relative;

			text-decoration: none;
			cursor: pointer;

			//span
			span {
				z-index: 1;
				position: relative;
				color: var(--clr-black);
				text-shadow:
					-1px -1px 0 var(--clr-white),
					1px -1px 0 var(--clr-white),
					-1px 1px 0 var(--clr-white),
					1px 1px 0 var(--clr-white);
				text-wrap: nowrap;
			}

			//underline
			&:after {
				display: block;
				z-index: 0;
				position: absolute;
				right: 0;
				bottom: 0;
				width: 100%;
				height: 1px;
				transform: scaleY(0.5);
				transform-origin: right;
				background-color: var(--clr-black);
				content: '';
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
				transform: translate(0, 0);
				opacity: 1;
			}
		}
	}

	.links-list {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: flex-start;
		gap: 26px;

		@media (max-width: 900px) {
			&:first-child::after {
				display: block;
				width: 100%;
				height: 1px;
				background-color: var(--clr-gray);
				content: '';
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

<a class="banner" href="https://merge.berlin" target="_blank">
	<div class="wrap">
		<img class="logo" alt="" src="/images/the-merge-banner/the-merge-logo.svg" />
		<span class="text"
			>Don't miss <strong>The Merge</strong>, the world's best developer experience conference!</span
		>
		<span class="text">Berlin, June 13 & 14, 2024</span>
	</div>
</a>

<style>
	.banner {
		display: flex;
		align-items: center;
		width: 100%;
		background-image: url('/images/the-merge-banner/banner-bg.svg');
		background-position: center;
		background-size: 100%;
		background-color: #24223c;

		transition: background-size 0.5s ease-in-out;
	}

	.banner:hover {
		background-size: 103%;
	}

	/* apply color to text on hover */
	.banner:hover span {
		color: #fff;
	}

	.wrap {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		max-width: 64rem;
		margin: 0 auto;
		padding: 1rem 0.75rem;
		gap: 30px;
	}

	.logo {
		height: 32px;
	}

	.text {
		/* flex: 1; */
		line-height: 1.2;
		transition: color 0.5s ease-in-out;
	}

	@media (max-width: 600px) {
		.banner {
			background-size: cover;
		}

		.wrap {
			flex-direction: column;
			align-items: flex-start;
			gap: 12px;
		}

		.logo {
			height: 32px;
		}

		.text {
			color: #ffffffa4;
		}
	}
</style>

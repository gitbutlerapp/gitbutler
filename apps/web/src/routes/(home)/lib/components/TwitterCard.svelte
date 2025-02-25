<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		tweet: {
			authorName: string;
			authorHandle: string;
			authorAvatar: string;
			content: string;
			date: string;
			link: string;
		};
	}

	let { tweet }: Props = $props();

	let textElement = $state<HTMLParagraphElement>();

	onMount(() => {
		if (textElement) {
			const text = textElement.innerText;
			const textArray = text.split(' ');

			function handleLink(word: string) {
				const isHandle = word.startsWith('@') || word.startsWith('#');

				if (isHandle) {
					return `<span style="color: #356FDF">${word}</span>`;
				}

				return word;
			}

			const textWithLinks = textArray.map((word) => handleLink(word)).join(' ');

			textElement.innerHTML = textWithLinks;
		}
	});
</script>

<a class="twitter-card" href={tweet.link} target="_blank">
	<div class="bio-wrap">
		<img class="avatar" src={tweet.authorAvatar} alt="" />
		<div class="bio">
			<h4 class="author-name">{tweet.authorName}</h4>
			<span class="author-handle">{tweet.authorHandle}</span>
		</div>
		<svg width="17" height="17" viewBox="0 0 17 17" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path
				d="M10.1173 7.19736L16.4459 -0.000976562H14.9463L9.45111 6.24924L5.06215 -0.000976562H0L6.63697 9.45046L0 16.999H1.49977L7.30279 10.3986L11.9379 16.999H17L10.1169 7.19736H10.1173ZM8.06317 9.53373L2.04016 1.10375H4.34371L14.947 15.9445H12.6434L8.06317 9.53409V9.53373Z"
				fill="black"
			/>
		</svg>
	</div>
	<div class="content">
		<p class="text" bind:this={textElement}>
			{tweet.content}
		</p>
		<span class="date">{tweet.date}</span>
	</div>
	<div class="divider"></div>
	<div class="cta">Read more on X</div>
</a>

<style lang="scss">
	.twitter-card {
		scroll-snap-align: center;
		z-index: 1;
		flex: 1;
		user-select: none;
		display: flex;
		flex-direction: column;
		background-color: var(--clr-white);
		padding: 24px;
		border-radius: 20px;
		text-decoration: none;
		transition:
			transform 0.1s ease-in-out,
			box-shadow 0.1s ease-in-out,
			opacity 0.1s ease-in-out;

		@media (min-width: 1100px) {
			flex: none;

			&:hover {
				z-index: 2;
				transform: translate(0, -6px);
				box-shadow: 0 16px 16px rgba(0, 0, 0, 0.1);
			}
		}
	}

	.bio-wrap {
		display: flex;
		flex-direction: row;
		gap: 16px;
		margin-bottom: 16px;
	}

	.bio {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.avatar {
		width: 44px;
		height: 44px;
		border-radius: 50%;
	}

	.author-name {
		font-family: Arial, Helvetica, sans-serif;
		font-weight: 700;
		font-size: 18px;
		color: var(--clr-black);
		margin: 0;
		margin-bottom: 3px;
	}

	.author-handle {
		font-family: Arial, Helvetica, sans-serif;
		font-weight: 400;
		font-size: 14px;
		color: var(--clr-black);
		opacity: 0.5;
		margin: 0;
	}

	.content {
		display: flex;
		flex-direction: column;
	}

	.text {
		font-family: Arial, Helvetica, sans-serif;
		font-weight: 400;
		font-size: 16px;
		line-height: 1.3;
		color: var(--clr-black);
		margin: 0;
		margin-bottom: 8px;
	}

	.date {
		font-family: Arial, Helvetica, sans-serif;
		font-weight: 400;
		font-size: 13px;
		color: var(--clr-black);
		opacity: 0.5;
		margin: 0;
	}

	.divider {
		height: 1px;
		background-color: var(--clr-gray);
		margin: 16px 0;
		opacity: 0.8;
	}

	.cta {
		font-family: Arial, Helvetica, sans-serif;
		font-weight: 700;
		font-size: 14px;
		color: var(--clr-black);
		text-align: center;
		text-decoration: none;
		margin: 0;
		border-radius: 40px;
		padding: 8px 16px;
		border: 1px solid var(--clr-gray);
	}
</style>

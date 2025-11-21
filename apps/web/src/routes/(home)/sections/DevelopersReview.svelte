<script lang="ts">
	// import TwitterCard from '$home/components/TwitterCard.svelte';
</script>

<section class="reviews-wrapper" id="developers-preview">
	<h2 class="title">What <i>developers<br />say</i> about us</h2>
	<section class="reviews">
		<!-- <div class="reviews-column">
			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/1764542513098928129/TLwqKsjH_200x200.jpg',
					authorName: 'Carlo',
					authorHandle: '@carlogilmar',
					content:
						"I'm really careful with my commits, I use to make it atomic and try to make in independent. With a good git knowledge I'm able to squash the commits that are related and keep branches with atomic changes, like legos bricks, but you need to do magic with git. Love this tool!",
					date: '9:07 PM · Jul 6, 2023',
					link: 'https://x.com/carlogilmar/status/1677031542634299392'
				}}
			/>
			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/1724693763132420096/ek5IOaN5_200x200.jpg',
					authorName: 'Pedro Piñera',
					authorHandle: '@pepicrft',
					content: 'The design looks gorgeous 😍',
					date: '8:27 PM · Jan 18, 2024',
					link: 'https://x.com/pepicrft/status/1748064672585023910'
				}}
			/>
		</div>
		<div class="reviews-column">
			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/848194327272730630/5YtMSV-J_200x200.jpg',
					authorName: 'Joel Califa',
					authorHandle: '@notdetails',
					content: 'Very cool approach to stacked diffs!',
					date: '4:57 PM · Jul 6, 2023',
					link: 'https://x.com/notdetails/status/1676968589050662915'
				}}
			/>

			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/1320428269791969280/g7bpc7JY_200x200.jpg',
					authorName: 'Derek Neighbors',
					authorHandle: '@dneighbors',
					content:
						'You might want to follow along with @gitbutler if you are looking for something fresh',
					date: '4:241 AM · Jan 25, 2024',
					link: 'https://x.com/dneighbors/status/1750363117446406148'
				}}
			/>
		</div>
		<div class="reviews-column">
			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/1678405458384527365/rCggBIjK_200x200.jpg',
					authorName: 'Paul Stack',
					authorHandle: '@stack72',
					content:
						'FWIW, @gitbutler has absolutely changed my git workflow to make it easier to apply those PRs to my environment! If you’ve not seen it before then check it out',
					date: '12:57 AM · Dec 31, 2023',
					link: 'https://twitter.com/stack72/status/1741247110886969671'
				}}
			/>
			<TwitterCard
				tweet={{
					authorAvatar:
						'https://pbs.twimg.com/profile_images/1384539886980681738/XIT9pYbY_200x200.jpg',
					authorName: 'Peter Pistorius',
					authorHandle: '@appfactory',
					content: 'This is really cool!',
					date: '7:44 PM · Jul 6, 2023',
					link: 'https://x.com/appfactory/status/1677010531297681409'
				}}
			/>
		</div> -->
	</section>
</section>

<style lang="scss">
	.reviews-wrapper {
		display: flex;
		flex-direction: column;
		margin-bottom: 80px;
		padding: 56px 60px;
		overflow: hidden;
		gap: 46px;
		border-radius: 20px;
		background-color: var(--clr-accent);

		@media (max-width: 1100px) {
			padding: 50px 30px;
			gap: 36px;
		}

		@media (max-width: 800px) {
			margin: 0 -30px 60px;
			border-radius: 0;
		}
	}

	.title {
		color: var(--clr-black);
		font-weight: 400;
		font-size: 70px;
		line-height: 90%;
		font-family: var(--font-accent);

		@media (max-width: 600px) {
			// csslint ignore
			font-size: 16cqw;
		}
	}

	.reviews {
		display: grid;
		position: relative;
		grid-template-rows: masonry;
		grid-template-columns: repeat(3, 1fr);
		margin-bottom: -145px;
		gap: 20px;

		@media (max-width: 1100px) {
			display: flex;
			align-items: flex-start;
			margin: 0 -30px;
			padding: 0 30px;
			overflow-x: scroll;
			scroll-snap-type: x mandatory;
		}

		// hide scrollbar
		&::-webkit-scrollbar {
			display: none;
		}
	}

	// .reviews-column {
	// 	display: flex;
	// 	flex-direction: column;
	// 	gap: 20px;

	// 	@media (max-width: 1100px) {
	// 		flex-direction: row;
	// 		align-items: flex-start;
	// 		min-width: 600px;
	// 	}
	// }
</style>

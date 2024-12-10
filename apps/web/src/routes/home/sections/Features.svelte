<script>
	import FeatureCard from '../components/FeatureCard.svelte';
	import MobileVideoDemoThumb from '../components/MobileVideoDemoThumb.svelte';
</script>

<section class="features" id="features">
	<FeatureCard
		title="Virtual Branches"
		description="You don’t need to switch branches if you can work on several simultaneously. Fix your bug while you work on your feature."
		posterUrl="/images/features/vb-poster.webp"
		videoUrl="https://d3brppdydubvmf.cloudfront.net/videos/web/feature-vb.mp4"
		readMoreUrl="#faq"
	/>
	<FeatureCard
		title="Branch Management"
		description="Undo, squash and amend your work by just dragging and dropping. No need to wrestle with rebase -i."
		posterUrl="/images/features/vb-managment-poster.webp"
		videoUrl="https://d3brppdydubvmf.cloudfront.net/videos/web/feature-vb-manage.mp4"
		readMoreUrl="#faq"
	/>
	<FeatureCard
		title="Quick Commits"
		description="How many times have you written git commit -m stuff? Generate a commit message with one click."
		posterUrl="/images/features/vb-commits-poster.webp"
		videoUrl="https://d3brppdydubvmf.cloudfront.net/videos/web/feature-vb-commit.mp4"
		readMoreUrl="#faq"
	/>
	<MobileVideoDemoThumb />
</section>

<style lang="scss">
	.features {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: 20px;
		padding-bottom: 60px;

		@media (max-width: 1100px) {
			grid-template-columns: repeat(2, 1fr);
		}

		@media (max-width: 800px) {
			grid-template-columns: repeat(1, 1fr);
		}
	}
</style>

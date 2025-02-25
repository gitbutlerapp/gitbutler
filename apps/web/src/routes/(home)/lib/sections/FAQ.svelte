<script>
	import FAQItem from '$home/lib/components/FAQItem.svelte';

	const faqItems = [
		{
			label: 'Is GitButler compatible with my existing repositories and workflow? ',
			content:
				'Yes, GitButler ultimately writes plain Git trees, which are pushed to a remote of your choice.',
			bgIndex: '0'
		},
		{
			label: 'Why build another Git client?',
			content:
				'We love Git. Instead of trying to fit the semantics of the Git CLI into a graphical interface, we wanted to start with the developer workflow and mapping it back to Git.',
			bgIndex: '1'
		},
		{
			label: 'Why should I use GitButler?',
			content:
				'GitButler allows you to organize your committed and uncommitted work in a way that you can easily create multiple branches (or Pull Requests) from it. It can also aid you as a code reviewer by making trying out remote branches more convenient.',
			bgIndex: '2'
		},
		{
			label: "What is GitButler's business model?",
			content:
				"GitButler's client is free for individual developers. In the future, we plan to offer team features aimed at companies.",
			bgIndex: '3'
		},
		{
			label: 'Can I use GitButler in the terminal?',
			content: 'Not yet. However, we plan to support a terminal interface in the future.',
			bgIndex: '4'
		},
		{
			label: 'What are virtual branches under the hood?',
			content:
				'Virtual branches are derived from the working directory, not the other way around. This allows us to work with a solid set of guarantees surrounding conflict resolution, merging and splitting up of hunks.',
			bgIndex: '0'
		}
	];

	let isOpen = false;
</script>

<section class="faq-section" id="faq">
	<h2 class="title">FAQ</h2>
	<section class="faq-wrapper">
		{#each faqItems as faqItem}
			<FAQItem {faqItem} />
		{/each}
	</section>
</section>

<style lang="scss">
	.faq-section {
		display: flex;
		flex-direction: column;
		margin-bottom: 60px;
	}

	.title {
		font-family: 'PP Editorial New', serif;
		font-weight: 400;
		font-size: 70px;
		color: var(--clr-black);
		line-height: 90%;
		margin-bottom: 30px;
	}

	.faq-wrapper {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
</style>

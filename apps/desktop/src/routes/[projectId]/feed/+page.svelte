<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Interest from '@gitbutler/shared/redux/interest/Interest.svelte';
	import { FeedService } from '@gitbutler/shared/redux/posts/service';
	import { feedSelectors } from '@gitbutler/shared/redux/posts/slice';
	import { useStore } from '@gitbutler/shared/redux/utils';

	const store = useStore();
	const feedService = getContext(FeedService);

	const feedInterest = feedService.getFeedInterest();
	const feed = $derived(feedSelectors.selectById($store, 'all'));

	$inspect(feed);
</script>

<Interest interest={feedInterest} />

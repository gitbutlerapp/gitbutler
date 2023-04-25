<script lang="ts">
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { Button } from '$lib/components';
	import { IconArrowLeft, IconArrowRight } from './icons';

	const getUri = (url: URL) => url.pathname + url.search + url.hash;

	let position = 0;
	const history = [getUri($page.url)];

	$: canGoBack = history.length > 1 && position > 0;
	$: canGoForward = history.length > 1 && position < history.length - 1;

	afterNavigate((nav) => {
		if (nav.to === null) return;
		const to = getUri(nav.to.url);
		if (to === history[position]) {
			return;
		} else if (to === history[position + 1]) {
			position++;
		} else if (to === history[position - 1]) {
			position--;
		} else {
			history.splice(position + 1);
			history.push(to);
			position++;
		}
	});

	const onBackClicked = () => {
		if (canGoBack) {
			position--;
			goto(history[position]);
		}
	};

	const onForwardClicked = () => {
		if (canGoForward) {
			position++;
			goto(history[position]);
		}
	};
</script>

<div class="flex items-center justify-center space-x-3 text-zinc-600">
	<Button filled={false} on:click={onBackClicked} disabled={!canGoBack} icon={IconArrowLeft} />
	<Button
		filled={false}
		on:click={onForwardClicked}
		icon={IconArrowRight}
		disabled={!canGoForward}
	/>
</div>

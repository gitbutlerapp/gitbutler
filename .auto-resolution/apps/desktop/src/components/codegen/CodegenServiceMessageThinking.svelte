<script lang="ts">
	import CodegenServiceMessage from '$components/codegen/CodegenServiceMessage.svelte';

	type Props = {
		lastUserMessageSent: Date;
		msSpentWaiting: number;
	};

	const { lastUserMessageSent, msSpentWaiting }: Props = $props();

	const words = [
		'contemplating',
		'reflecting',
		'reasoning',
		'analyzing',
		'pondering',
		'deliberating',
		'mulling',
		'meditating',
		'speculating',
		'conceptualizing',
		'building',
		'assembling',
		'creating',
		'developing',
		'forming',
		'fabricating',
		'composing',
		'establishing',
		'designing',
		'operating',
		'functioning',
		'executing',
		'acting',
		'butlering',
		'producing',
		'laboring',
		'performing',
		'engaging',
		'applying',
		'striving'
	];

	/**
	 * Securely generates a number between 0 and limit (inclusive)
	 */
	function randomInt(limit: number) {
		const array = new Uint32Array(1);
		crypto.getRandomValues(array);
		const randFloat = array[0]! / (0xffffffff + 1);
		return Math.round(randFloat * limit);
	}

	function getWord() {
		const i = randomInt(words.length - 1);
		return words[i];
	}

	function milisToEnglish(milis: number) {
		if (milis === 0) return 'now';

		const seconds = milis / 1000;
		const minutes = Math.floor(seconds / 60);
		const hours = Math.floor(minutes / 60);

		if (hours > 0) return `${hours}h ${minutes % 60}m`;
		if (minutes > 0) return `${minutes}m ${(seconds % 60).toFixed(1)}s`;
		return `${seconds.toFixed(1)}s`;
	}

	let currentWord = $state(getWord());
	let currentDuration = $state(
		milisToEnglish(Date.now() - lastUserMessageSent.getTime() - msSpentWaiting)
	);

	$effect(() => {
		const updateWordInterval = setInterval(() => {
			currentWord = getWord();
		}, 1000 * 15);

		const updateTimeInterval = setInterval(() => {
			currentDuration = milisToEnglish(Date.now() - lastUserMessageSent.getTime() - msSpentWaiting);
		}, 100);

		return () => {
			clearInterval(updateWordInterval);
			clearInterval(updateTimeInterval);
		};
	});
</script>

<CodegenServiceMessage style="neutral" face="thinking">
	<span class="text-13 text-italic">
		Claude is
		{#key currentWord}
			<span class="animated-word">{currentWord}</span>
		{/key}
		... {currentDuration}
	</span>
</CodegenServiceMessage>

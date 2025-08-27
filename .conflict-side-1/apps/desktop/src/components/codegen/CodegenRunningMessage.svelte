<script lang="ts">
	import CodegenMessage from '$components/codegen/CodegenMessage.svelte';
	import { AsyncButton } from '@gitbutler/ui';

	type Props = {
		lastUserMessageSent: Date;
		onAbort: () => Promise<void>;
	};

	const { lastUserMessageSent, onAbort }: Props = $props();

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
		'erecting',
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
		const seconds = Math.floor(milis / 1000) % 60;
		const minutes = Math.floor(milis / (1000 * 60)) % 60;
		const hours = Math.floor(milis / (1000 * 60 * 60)) % 60;
		let out = '';
		if (hours !== 0) out += `${hours}h `;
		if (minutes !== 0) out += `${minutes}m `;
		if (seconds !== 0) out += `${seconds}s`;
		return out.trim();
	}

	function getUTCNow() {
		const time = new Date();
		const offset = time.getTimezoneOffset();
		return Date.now() + offset * 1000 * 60;
	}

	let currentWord = $state(getWord());
	let currentDuration = $state(milisToEnglish(getUTCNow() - lastUserMessageSent.getTime()));

	$effect(() => {
		const updateWordInterval = setInterval(() => {
			currentWord = getWord();
		}, 1000 * 15);

		const updateTimeInterval = setInterval(() => {
			currentDuration = milisToEnglish(getUTCNow() - lastUserMessageSent.getTime());
		}, 1000);

		return () => {
			clearInterval(updateWordInterval);
			clearInterval(updateTimeInterval);
		};
	});
</script>

<CodegenMessage content="Claude is {currentWord}... {currentDuration}" side="left" bubble>
	{#snippet extraContent()}
		<AsyncButton kind="outline" action={onAbort}>Abort</AsyncButton>
	{/snippet}
</CodegenMessage>

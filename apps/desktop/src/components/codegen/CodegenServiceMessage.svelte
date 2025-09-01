<script lang="ts">
	import { AgentAvatar } from '@gitbutler/ui';
	import { fade } from 'svelte/transition';

	type Props = {
		lastUserMessageSent: Date;
	};

	const { lastUserMessageSent }: Props = $props();

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

		const seconds = milis / 1000;
		const minutes = Math.floor(seconds / 60);
		const hours = Math.floor(minutes / 60);

		if (hours > 0) return `${hours}h ${minutes % 60}m`;
		if (minutes > 0) return `${minutes}m ${(seconds % 60).toFixed(1)}s`;
		return `${seconds.toFixed(1)}s`;
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
		}, 100);

		return () => {
			clearInterval(updateWordInterval);
			clearInterval(updateTimeInterval);
		};
	});
</script>

<div class="service-message__wrapper">
	<div class="service-message">
		<AgentAvatar />
		<div class="service-message__bubble">
			<span class="text-13 text-italic">
				Claude is
				{#key currentWord}
					<span class="animated-word" in:fade={{ duration: 150 }}>{currentWord}</span>
				{/key}
				... {currentDuration}
			</span>
		</div>
	</div>
</div>

<style lang="postcss">
	.service-message__wrapper {
		display: flex;
		width: 100%;
		padding: 8px 0 16px;
	}
	.service-message {
		display: flex;
		align-items: flex-end;
		gap: 16px;
	}
	.service-message__bubble {
		display: flex;
		max-width: var(--message-max-width);
		padding: 8px 12px;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}
</style>

import { debouncePromise } from '$lib/utils/misc';

async function playSoundImpl(soundUrl: string) {
	const audio = new Audio(soundUrl);
	await audio.play();
}

export const playSound = debouncePromise(playSoundImpl, 1000);

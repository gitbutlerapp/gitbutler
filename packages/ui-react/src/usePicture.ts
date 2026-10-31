import { gravatarUrl, withoutGeneratedFace } from "./gravatar.ts";
import { personColour } from "./personColour.ts";
import { useState } from "react";

/**
 * What to draw for a person: their picture, else the real Gravatar photo for an email seed,
 * else nothing, and the caller shows the person's glitch on `tint`, as it does under a
 * picture while it loads. Generated faces are never asked for, so no one gets an 8-bit stand-in. It
 * remembers the sources that failed rather than a flag, so a new picture gets its own try
 * without an effect to reset anything.
 */
export const usePicture = (
	src: string | null | undefined,
	seed: string,
	size: number,
): { url: string | null; tint: string; onError: () => void } => {
	const [failed, setFailed] = useState<ReadonlyArray<string>>([]);
	const candidates = [
		src == null || src === "" ? null : withoutGeneratedFace(src),
		seed.includes("@") ? gravatarUrl(seed, size) : null,
	].filter((url): url is string => url != null && url !== "");
	const url = candidates.find((candidate) => !failed.includes(candidate)) ?? null;
	return {
		url,
		tint: personColour(seed).ground,
		onError: () => url !== null && setFailed((was) => [...was, url]),
	};
};

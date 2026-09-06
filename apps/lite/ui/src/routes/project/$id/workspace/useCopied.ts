import { useEffect, useRef, useState } from "react";

/**
 * Copies `value` to the clipboard on `copy`, and says so for a moment:
 * `copied` holds for a second and a half after each copy, for the control
 * to show in place of its usual text.
 */
export const useCopied = (value: string): { copied: boolean; copy: () => void } => {
	const [copied, setCopied] = useState(false);
	const resetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	const copy = () => {
		void window.lite.clipboardWriteText(value);
		setCopied(true);

		if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		resetTimeoutRef.current = setTimeout(() => setCopied(false), 1500);
	};

	useEffect(
		() => () => {
			if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		},
		[],
	);

	return { copied, copy };
};

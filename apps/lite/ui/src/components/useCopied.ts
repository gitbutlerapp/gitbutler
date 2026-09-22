import { useCopied as useCopiedWith } from "@gitbutler/ui-react/useCopied.ts";

/** The library's `useCopied`, writing through Electron's clipboard rather than the page's. */
export const useCopied = (value: string): { copied: boolean; copy: () => void } =>
	useCopiedWith(value, (text) => window.lite.clipboardWriteText(text));

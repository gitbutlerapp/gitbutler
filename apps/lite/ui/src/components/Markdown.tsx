import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { openLinkExternally } from "#ui/external-link.ts";
import { Markdown as BaseMarkdown } from "@gitbutler/ui-react/Markdown.tsx";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";

/**
 * The library's Markdown with the app plugged in: links leave through the
 * system browser, code blocks copy through Electron's clipboard, and fenced
 * code takes the syntax theme pair from the settings, as the diff viewer does.
 * @import import { Markdown } from "#ui/components/Markdown.tsx";
 */
export const Markdown: FC<{ children: string }> = ({ children }) => {
	const { data: themes } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.syntaxHighlighting,
	});

	return (
		<BaseMarkdown
			onOpenLink={openLinkExternally}
			copyText={(text) => window.lite.clipboardWriteText(text)}
			highlightThemes={{
				light: themes?.light ?? defaultSettings.syntaxHighlighting.light,
				dark: themes?.dark ?? defaultSettings.syntaxHighlighting.dark,
			}}
		>
			{children}
		</BaseMarkdown>
	);
};

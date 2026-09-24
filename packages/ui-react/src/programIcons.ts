import alacritty from "./program-icons/alacritty@2x.png";
import cursor from "./program-icons/cursor@2x.png";
import ghostty from "./program-icons/ghostty@2x.png";
import hyper from "./program-icons/hyper@2x.png";
import iterm2 from "./program-icons/iterm2@2x.png";
import kitty from "./program-icons/kitty@2x.png";
import sublime from "./program-icons/sublime@2x.png";
import terminal from "./program-icons/terminal@2x.png";
import vscode from "./program-icons/vscode@2x.png";
import warp from "./program-icons/warp@2x.png";
import wezterm from "./program-icons/wezterm@2x.png";
import zed from "./program-icons/zed@2x.png";

/**
 * The marks of the editors and terminals the app can open things in, keyed by the
 * identifier the backend lists the program under.
 *
 * These are images rather than icons: a program's mark carries its own colours and
 * detail, so it cannot be a monochrome glyph that inherits the text colour the way
 * everything in `icons/` does. They are exported from ⚛️ Core at twice their
 * rendered size, which is what keeps them sharp on a retina display.
 *
 * Listed by hand, as `illustrations.ts` is, so a mark nobody renders shows up as an
 * unused key.
 */
const programIcons = {
	alacritty,
	cursor,
	ghostty,
	hyper,
	iterm2,
	kitty,
	sublime,
	terminal,
	vscode,
	warp,
	wezterm,
	zed,
} as const;

type ProgramIconName = keyof typeof programIcons;

/** Identifiers that name the same program packaged for another platform. */
const aliases: Record<string, ProgramIconName> = {
	"alacritty-mac": "alacritty",
	"wezterm-mac": "wezterm",
};

const isProgramIconName = (name: string): name is ProgramIconName => name in programIcons;

/** The mark for a program identifier, or nothing when the program has none. */
export const programIconFor = (program: string): string | undefined => {
	const name = aliases[program] ?? program;
	return isProgramIconName(name) ? programIcons[name] : undefined;
};

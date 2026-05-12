import { createContext, FC, ReactNode, useContext } from "react";
import { createPortal } from "react-dom";

export const TopBarActionsElementContext = createContext<HTMLElement | undefined>(undefined);
export const ShortcutsBarElementContext = createContext<HTMLElement | undefined>(undefined);

export const TopBarActionsPortal: FC<{ children: ReactNode }> = ({ children }) => {
	const element = useContext(TopBarActionsElementContext);
	if (!element) return null;

	return createPortal(children, element);
};

export const ShortcutsBarPortal: FC<{ children: ReactNode }> = ({ children }) => {
	const element = useContext(ShortcutsBarElementContext);
	if (!element) return null;

	return createPortal(children, element);
};

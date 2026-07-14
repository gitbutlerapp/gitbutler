import { createContext } from "react";

type DetailsFullWindowContext = {
	detailsFullWindow: boolean;
	setDetailsFullWindow: (fullWindow: boolean) => void;
	toggleDetailsFullWindow: () => void;
};

export const DetailsFullWindowContext = createContext({} as DetailsFullWindowContext);
DetailsFullWindowContext.displayName = "DetailsFullWindowContext";
